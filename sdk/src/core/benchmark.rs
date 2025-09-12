use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tracing::info;

struct StreamState {
    // concurrent process_binding tasks in this stream
    active: i32,
    // stream lifetime start
    open_at: Instant,
    // start of current working interval if active
    work_start: Option<Instant>,
    // cumulative working time across lifetime (ns)
    work_ns_total: u128,
    closed: bool,
}

impl Default for StreamState {
    fn default() -> Self {
        Self { active: 0, open_at: Instant::now(), work_start: None, work_ns_total: 0, closed: false }
    }
}

struct AccumState {
    streams: HashMap<i32, StreamState>,
}

struct BenchmarkInner {
    state: AccumState,
}

impl BenchmarkInner {
    fn on_binding_inc(&mut self, stream_id: i32) {
        let now = Instant::now();
        let st = self
            .state
            .streams
            .entry(stream_id)
            .or_insert_with(StreamState::default);
        if st.active == 0 {
            st.work_start = Some(now);
        }
        st.active += 1;
    }

    fn on_binding_dec(&mut self, stream_id: i32) {
        let now = Instant::now();
        if let Some(st) = self.state.streams.get_mut(&stream_id) {
            st.active -= 1;
            if st.active < 0 {
                st.active = 0;
            }
            if st.active == 0 {
                if let Some(ws) = st.work_start.take() {
                    st.work_ns_total += now.saturating_duration_since(ws).as_nanos();
                }
            }
        } else {
            self.state.streams.insert(stream_id, StreamState::default());
        }
    }

    fn on_stream_open(&mut self, stream_id: i32) {
        let now = Instant::now();
        // Insert if absent
        self.state
            .streams
            .entry(stream_id)
            .or_insert_with(|| StreamState { active: 0, open_at: now, work_start: None, work_ns_total: 0, closed: false });
        // Update global stream concurrency atomically
        let active = STREAMS_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
        // max
        let _ = STREAMS_MAX_ACTIVE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            if active > cur { Some(active) } else { None }
        });
        // min
        let _ = STREAMS_MIN_ACTIVE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            if active < cur { Some(active) } else { None }
        });
    }

    fn on_stream_close(&mut self, stream_id: i32) {
        let now = Instant::now();
        if let Some(st) = self.state.streams.get_mut(&stream_id) {
            if st.active > 0 {
                if let Some(ws) = st.work_start.take() {
                    st.work_ns_total += now.saturating_duration_since(ws).as_nanos();
                }
                st.active = 0;
            }
            st.closed = true;
        }
        let active = STREAMS_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
        if active < 0 {
            STREAMS_ACTIVE.store(0, Ordering::Relaxed);
        }
        let _ = STREAMS_MIN_ACTIVE.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            if active < cur { Some(active) } else { None }
        });
    }

    fn record_handler_time(&mut self, dur: Duration) {
        let ns = dur.as_nanos();
        HANDLER_TIME_NS_SUM.fetch_add((ns.min(u64::MAX as u128)) as u64, Ordering::Relaxed);
        HANDLER_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    fn record_db_time(&mut self, dur: Duration) {
        let ns = dur.as_nanos();
        DB_TIME_NS_SUM.fetch_add((ns.min(u64::MAX as u128)) as u64, Ordering::Relaxed);
        DB_OPS.fetch_add(1, Ordering::Relaxed);
    }

    fn drain_and_report(&mut self) {
        let now_inst = Instant::now();
        let now_ms = now_epoch_ms();
        let last_ms = WINDOW_START_EPOCH_MS.swap(now_ms, Ordering::AcqRel);
        let window_ms = (now_ms.saturating_sub(last_ms)).max(1) as f64;

        // Working time is accounted on transitions; no per-tick accrual needed.

        let handler_calls = HANDLER_CALLS.swap(0, Ordering::AcqRel);
        let handler_time_ns_sum = HANDLER_TIME_NS_SUM.swap(0, Ordering::AcqRel) as f64;
        let avg_handle_ms = if handler_calls > 0 {
            handler_time_ns_sum / (handler_calls as f64) / 1_000_000.0
        } else {
            0.0
        };
        let db_ops = DB_OPS.swap(0, Ordering::AcqRel);
        let db_time_ns_sum = DB_TIME_NS_SUM.swap(0, Ordering::AcqRel) as f64;
        let avg_db_ms = if db_ops > 0 {
            db_time_ns_sum / (db_ops as f64) / 1_000_000.0
        } else {
            0.0
        };

        let receive_time_ns_sum = RECEIVE_TIME_NS_SUM.swap(0, Ordering::AcqRel) as f64;
        let receive_count = RECEIVE_COUNT.swap(0, Ordering::AcqRel);
        let avg_receive_ms = if receive_count > 0 {
            receive_time_ns_sum / (receive_count as f64) / 1_000_000.0
        } else {
            0.0
        };

        // Print header (streams concurrency)
        info!(
            "[BENCH] last {:.0}s | avg handle {:.3}ms | calls {} | avg db {:.3}ms | avg recv {:.3}ms | streams max {} min {}",
            window_ms / 1000.0,
            avg_handle_ms,
            handler_calls,
            avg_db_ms,
            avg_receive_ms,
            STREAMS_MAX_ACTIVE.load(Ordering::Relaxed),
            STREAMS_MIN_ACTIVE.load(Ordering::Relaxed),
        );

        // Per-stream work summary (lifetime-based)
        for (stream_id, st) in self.state.streams.iter() {
            let lifetime_ms = now_inst.saturating_duration_since(st.open_at).as_millis() as f64;
            let mut work_ns = st.work_ns_total;
            if st.active > 0 {
                if let Some(ws) = st.work_start {
                    work_ns += now_inst.saturating_duration_since(ws).as_nanos();
                }
            }
            let work_ms = (work_ns as f64) / 1_000_000.0;
            let work_pct = if lifetime_ms > 0.0 { (work_ms / lifetime_ms) * 100.0 } else { 0.0 };
            info!(
                "[BENCH] stream {}: work {:.1}s ({:.1}%)",
                stream_id,
                work_ms / 1000.0,
                work_pct.max(0.0).min(100.0)
            );
        }

        // Reset stream concurrency window bounds to current active
        let active = STREAMS_ACTIVE.load(Ordering::Relaxed);
        STREAMS_MIN_ACTIVE.store(active, Ordering::Relaxed);
        STREAMS_MAX_ACTIVE.store(active.max(0), Ordering::Relaxed);

        // Remove closed streams after reporting to prevent leaks
        let closed_ids: Vec<i32> = self
            .state
            .streams
            .iter()
            .filter_map(|(id, st)| if st.closed { Some(*id) } else { None })
            .collect();
        for id in closed_ids {
            self.state.streams.remove(&id);
        }

        // Do not reset per-stream lifetime working time; keep accumulating until stream closed.
    }
}

// Global hot-path counters (lock-free)
static HANDLER_TIME_NS_SUM: AtomicU64 = AtomicU64::new(0);
static HANDLER_CALLS: AtomicU64 = AtomicU64::new(0);
static DB_TIME_NS_SUM: AtomicU64 = AtomicU64::new(0);
static RECEIVE_TIME_NS_SUM: AtomicU64 = AtomicU64::new(0);
static RECEIVE_COUNT: AtomicU64 = AtomicU64::new(0);
static DB_OPS: AtomicU64 = AtomicU64::new(0);
static WINDOW_START_EPOCH_MS: AtomicU64 = AtomicU64::new(0);

// Stream concurrency metrics (lock-free)
static STREAMS_ACTIVE: AtomicI32 = AtomicI32::new(0);
static STREAMS_MIN_ACTIVE: AtomicI32 = AtomicI32::new(i32::MAX);
static STREAMS_MAX_ACTIVE: AtomicI32 = AtomicI32::new(0);

// Stream details (rarely touched by hot path, guarded by a local mutex)
static STREAMS_MAP: OnceLock<Mutex<BenchmarkInner>> = OnceLock::new();
static REPORTER_STARTED: OnceLock<()> = OnceLock::new();
static STREAM_ID_GEN: OnceLock<AtomicI32> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);

fn get_inner() -> &'static Mutex<BenchmarkInner> {
    STREAMS_MAP.get_or_init(|| Mutex::new(BenchmarkInner { state: AccumState { streams: HashMap::new() } }))
}

#[inline]
fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

pub fn init_if_enabled() {
    // Only start reporter when SHOW_BENCHMARK_RESULT is set
    if std::env::var("SHOW_BENCHMARK_RESULT").is_err() {
        return;
    }
    // Mark benchmark system enabled so hot-paths can bail fast when disabled
    ENABLED.store(true, Ordering::Relaxed);
    if REPORTER_STARTED.set(()).is_ok() {
        WINDOW_START_EPOCH_MS.store(now_epoch_ms(), Ordering::Relaxed);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let inner = get_inner();
                let mut guard = inner.lock().unwrap();
                guard.drain_and_report();
            }
        });
        info!("[BENCH] Benchmark reporter started (SHOW_BENCHMARK_RESULT set)");
    }
}

pub fn new_stream_id() -> i32 {
    if !ENABLED.load(Ordering::Relaxed) {
        return 0;
    }
    let idgen = STREAM_ID_GEN.get_or_init(|| AtomicI32::new(1));
    idgen.fetch_add(1, Ordering::Relaxed)
}

pub fn on_stream_open(stream_id: i32) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    if stream_id == 0 { return; }
    let inner = get_inner();
    let mut guard = inner.lock().unwrap();
    guard.on_stream_open(stream_id);
}

pub fn on_stream_close(stream_id: i32) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    if stream_id == 0 { return; }
    let inner = get_inner();
    let mut guard = inner.lock().unwrap();
    guard.on_stream_close(stream_id);
}

pub fn on_binding_spawn(stream_id: i32) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    if stream_id == 0 { return; }
    let inner = get_inner();
    let mut guard = inner.lock().unwrap();
    guard.on_binding_inc(stream_id);
}

pub fn on_binding_done(stream_id: i32) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    if stream_id == 0 { return; }
    let inner = get_inner();
    let mut guard = inner.lock().unwrap();
    guard.on_binding_dec(stream_id);
}

pub fn record_handler_time(dur: Duration) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    // hot path: update atomics directly
    let ns = dur.as_nanos();
    HANDLER_TIME_NS_SUM.fetch_add((ns.min(u64::MAX as u128)) as u64, Ordering::Relaxed);
    HANDLER_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_db_time(dur: Duration) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    let ns = dur.as_nanos();
    DB_TIME_NS_SUM.fetch_add((ns.min(u64::MAX as u128)) as u64, Ordering::Relaxed);
    DB_OPS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_receive_time(p0: Duration) {
    if !ENABLED.load(Ordering::Relaxed) { return; }
    let ns = p0.as_nanos();
    RECEIVE_COUNT.fetch_add(1, Ordering::Relaxed);
    RECEIVE_TIME_NS_SUM.fetch_add((ns.min(u64::MAX as u128)) as u64, Ordering::Relaxed);
}