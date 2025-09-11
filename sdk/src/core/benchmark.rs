use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use tracing::info;

struct StreamState {
    active: i32,
    last_change: Instant,
    idle_ms_acc: u128,
    closed: bool,
}

impl Default for StreamState {
    fn default() -> Self {
        Self { active: 0, last_change: Instant::now(), idle_ms_acc: 0, closed: false }
    }
}

struct AccumState {
    streams: HashMap<i32, StreamState>,
    // Global concurrent active streams across server
    streams_active: i32,
    streams_min_active: i32,
    streams_max_active: i32,
    handler_time_ns_sum: u128,
    handler_calls: u64,
    db_time_ns_sum: u128,
    db_ops: u64,
    window_start: Instant,
}

impl AccumState {
    fn new() -> Self {
        Self {
            streams: HashMap::new(),
            streams_active: 0,
            streams_min_active: i32::MAX,
            streams_max_active: 0,
            handler_time_ns_sum: 0,
            handler_calls: 0,
            db_time_ns_sum: 0,
            db_ops: 0,
            window_start: Instant::now(),
        }
    }
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
            .or_insert_with(|| StreamState {
                active: 0,
                last_change: now,
                idle_ms_acc: 0,
                closed: false,
            });
        // If previously idle, add idle duration until now
        if st.active == 0 {
            let dur = now.saturating_duration_since(st.last_change);
            st.idle_ms_acc += dur.as_millis();
        }
        st.active += 1;
        st.last_change = now;
    }

    fn on_binding_dec(&mut self, stream_id: i32) {
        let now = Instant::now();
        if let Some(st) = self.state.streams.get_mut(&stream_id) {
            st.active -= 1;
            if st.active < 0 {
                st.active = 0;
            }
            // If transitioned to idle, mark last_change for future idle accumulation
            if st.active == 0 {
                st.last_change = now;
            }
        } else {
            // Initialize and set to 0 idle
            self.state.streams.insert(
                stream_id,
                StreamState {
                    active: 0,
                    last_change: now,
                    idle_ms_acc: 0,
                    closed: false,
                },
            );
        }
    }

    fn on_stream_open(&mut self, stream_id: i32) {
        let now = Instant::now();
        // Insert if absent
        self.state.streams.entry(stream_id).or_insert_with(|| StreamState {
            active: 0,
            last_change: now,
            idle_ms_acc: 0,
            closed: false,
        });
        self.state.streams_active += 1;
        if self.state.streams_active > self.state.streams_max_active {
            self.state.streams_max_active = self.state.streams_active;
        }
        if self.state.streams_active < self.state.streams_min_active {
            self.state.streams_min_active = self.state.streams_active;
        }
    }

    fn on_stream_close(&mut self, stream_id: i32) {
        let now = Instant::now();
        if let Some(st) = self.state.streams.get_mut(&stream_id) {
            if st.active == 0 {
                let dur = now.saturating_duration_since(st.last_change);
                st.idle_ms_acc += dur.as_millis();
            }
            st.closed = true;
        }
        self.state.streams_active -= 1;
        if self.state.streams_active < 0 {
            self.state.streams_active = 0;
        }
        if self.state.streams_active < self.state.streams_min_active {
            self.state.streams_min_active = self.state.streams_active;
        }
    }

    fn record_handler_time(&mut self, dur: Duration) {
        self.state.handler_time_ns_sum += dur.as_nanos();
        self.state.handler_calls += 1;
    }

    fn record_db_time(&mut self, dur: Duration) {
        self.state.db_time_ns_sum += dur.as_nanos();
        self.state.db_ops += 1;
    }

    fn drain_and_report(&mut self) {
        let now = Instant::now();
        let window_dur = now.saturating_duration_since(self.state.window_start);
        let window_ms = window_dur.as_millis().max(1) as f64;

        // Top up idle time for streams that are currently idle
        for st in self.state.streams.values_mut() {
            if st.active == 0 {
                let dur = now.saturating_duration_since(st.last_change);
                st.idle_ms_acc += dur.as_millis();
                st.last_change = now;
            }
        }

        let avg_handle_ms = if self.state.handler_calls > 0 {
            (self.state.handler_time_ns_sum as f64) / (self.state.handler_calls as f64) / 1_000_000.0
        } else {
            0.0
        };
        let avg_db_ms = if self.state.db_ops > 0 {
            (self.state.db_time_ns_sum as f64) / (self.state.db_ops as f64) / 1_000_000.0
        } else {
            0.0
        };

        // Print header (streams concurrency)
        info!(
            "[BENCH] last {:.0}s | avg handle {:.3}ms | calls/min {} | avg db {:.3}ms | streams max {} min {}",
            window_ms / 1000.0,
            avg_handle_ms,
            self.state.handler_calls,
            avg_db_ms,
            self.state.streams_max_active,
            if self.state.streams_min_active == i32::MAX { 0 } else { self.state.streams_min_active },
        );

        // Per-stream idle summary
        for (stream_id, st) in self.state.streams.iter() {
            let idle_ms = st.idle_ms_acc as f64;
            let idle_pct = (idle_ms / window_ms) * 100.0;
            info!(
                "[BENCH] stream {}: idle {:.1}s ({:.1}%)",
                stream_id,
                idle_ms / 1000.0,
                idle_pct.max(0.0).min(100.0)
            );
        }

        // Reset window
        self.state.handler_time_ns_sum = 0;
        self.state.handler_calls = 0;
        self.state.db_time_ns_sum = 0;
        self.state.db_ops = 0;
        self.state.window_start = now;
        self.state.streams_min_active = self.state.streams_active;
        self.state.streams_max_active = self.state.streams_active.max(0);

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

        for st in self.state.streams.values_mut() {
            st.idle_ms_acc = 0;
            // last_change already set to now for idle streams; keep as-is for active
        }
    }
}

static BENCH_STATE: OnceLock<Arc<Mutex<BenchmarkInner>>> = OnceLock::new();
static REPORTER_STARTED: OnceLock<()> = OnceLock::new();
static STREAM_ID_GEN: OnceLock<AtomicI32> = OnceLock::new();

fn get_state() -> Arc<Mutex<BenchmarkInner>> {
    BENCH_STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(BenchmarkInner {
                state: AccumState::new(),
            }))
        })
        .clone()
}

pub fn init_if_enabled() {
    // Only start reporter when SHOW_BENCHMARK_RESULT is set
    if std::env::var("SHOW_BENCHMARK_RESULT").is_err() {
        return;
    }
    if REPORTER_STARTED.set(()).is_ok() {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let state = get_state();
                let mut guard = state.lock().unwrap();
                guard.drain_and_report();
            }
        });
        info!("[BENCH] Benchmark reporter started (SHOW_BENCHMARK_RESULT set)");
    }
}

pub fn new_stream_id() -> i32 {
    let idgen = STREAM_ID_GEN.get_or_init(|| AtomicI32::new(1));
    idgen.fetch_add(1, Ordering::Relaxed)
}

pub fn on_stream_open(stream_id: i32) {
    if stream_id == 0 { return; }
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.on_stream_open(stream_id);
}

pub fn on_stream_close(stream_id: i32) {
    if stream_id == 0 { return; }
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.on_stream_close(stream_id);
}

pub fn on_binding_spawn(stream_id: i32) {
    if stream_id == 0 { return; }
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.on_binding_inc(stream_id);
}

pub fn on_binding_done(stream_id: i32) {
    if stream_id == 0 { return; }
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.on_binding_dec(stream_id);
}

pub fn record_handler_time(dur: Duration) {
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.record_handler_time(dur);
}

pub fn record_db_time(dur: Duration) {
    let state = get_state();
    let mut guard = state.lock().unwrap();
    guard.record_db_time(dur);
}
