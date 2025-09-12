use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::info;

struct BenchState {
    api_total_ns: AtomicU64,
    api_count: AtomicU64,
    cache_hit_total_ns: AtomicU64,
    cache_hit_count: AtomicU64,
    decode_total_ns: AtomicU64,
    decode_count: AtomicU64,
    decode_failed_total_ns: AtomicU64,
    decode_failed_count: AtomicU64,
    handler_total_ns: AtomicU64,
    handler_count: AtomicU64,
    other_total_ns: AtomicU64,
    total_calls: AtomicU64,
    cache_keys: AtomicU64,
    last_print_epoch_sec: AtomicU64,
}

impl BenchState {
    fn new() -> Self {
        Self {
            api_total_ns: AtomicU64::new(0),
            api_count: AtomicU64::new(0),
            cache_hit_total_ns: AtomicU64::new(0),
            cache_hit_count: AtomicU64::new(0),
            decode_failed_total_ns: AtomicU64::new(0),
            decode_failed_count: AtomicU64::new(0),
            decode_total_ns: AtomicU64::new(0),
            decode_count: AtomicU64::new(0),
            handler_total_ns: AtomicU64::new(0),
            handler_count: AtomicU64::new(0),
            other_total_ns: AtomicU64::new(0),
            total_calls: AtomicU64::new(0),
            cache_keys: AtomicU64::new(0),
            last_print_epoch_sec: AtomicU64::new(now_secs()),
        }
    }
}

static ENABLED: OnceLock<bool> = OnceLock::new();
fn enabled() -> bool {
    *ENABLED
        .get_or_init(|| std::env::var("SHOW_BENCHMARK_RESULT").is_ok())
}

static STATE: OnceLock<BenchState> = OnceLock::new();
fn state() -> &'static BenchState {
    STATE.get_or_init(BenchState::new)
}

#[inline]
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[inline]
fn nanos_u64(dur: Duration) -> u64 {
    let n = dur.as_nanos();
    if n > u64::MAX as u128 { u64::MAX } else { n as u64 }
}

pub fn record_api_call(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.api_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
    s.api_count.fetch_add(1, Ordering::Relaxed);
}

pub fn record_cache_hit(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.cache_hit_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
    s.cache_hit_count.fetch_add(1, Ordering::Relaxed);
}

pub fn record_decode(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.decode_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
    s.decode_count.fetch_add(1, Ordering::Relaxed);
}

pub fn record_decode_failed(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.decode_failed_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
    s.decode_failed_count.fetch_add(1, Ordering::Relaxed);
}


pub fn record_handler(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.handler_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
    s.handler_count.fetch_add(1, Ordering::Relaxed);
}

pub fn record_other(dur: Duration) {
    if !enabled() {
        return;
    }
    let s = state();
    s.other_total_ns.fetch_add(nanos_u64(dur), Ordering::Relaxed);
}

pub fn mark_call() {
    if !enabled() {
        return;
    }
    let s = state();
    s.total_calls.fetch_add(1, Ordering::Relaxed);
}

pub fn update_cache_size(keys: u64) {
    if !enabled() {
        return;
    }
    let s = state();
    s.cache_keys.store(keys, Ordering::Relaxed);
}

pub fn print_if_due() {
    if !enabled() {
        return;
    }
    let s = state();
    let now = now_secs();
    let last = s.last_print_epoch_sec.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 60 {
        return;
    }
    if s
        .last_print_epoch_sec
        .compare_exchange(last, now, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        // Another thread is printing; skip.
        return;
    }

    let api_total_ns = s.api_total_ns.swap(0, Ordering::AcqRel) as f64;
    let api_count = s.api_count.swap(0, Ordering::AcqRel);
    let cache_hit_total_ns = s.cache_hit_total_ns.swap(0, Ordering::AcqRel) as f64;
    let cache_hit_count = s.cache_hit_count.swap(0, Ordering::AcqRel);
    let decode_total_ns = s.decode_total_ns.swap(0, Ordering::AcqRel) as f64;
    let decode_count = s.decode_count.swap(0, Ordering::AcqRel);
    let decode_failed_total_ns = s.decode_failed_total_ns.swap(0, Ordering::AcqRel) as f64;
    let decode_failed_count = s.decode_failed_count.swap(0, Ordering::AcqRel);
    let handler_total_ns = s.handler_total_ns.swap(0, Ordering::AcqRel) as f64;
    let handler_count = s.handler_count.swap(0, Ordering::AcqRel);
    let other_total_ns = s.other_total_ns.swap(0, Ordering::AcqRel) as f64;

    let avg_api_ms = if api_count > 0 {
        api_total_ns / (api_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_cache_ms = if cache_hit_count > 0 {
        cache_hit_total_ns / (cache_hit_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_decode_ms = if decode_count > 0 {
        decode_total_ns / (decode_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_decode_fail_ms = if decode_failed_count > 0 {
        decode_failed_total_ns / (decode_failed_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_handler_ms = if handler_count > 0 {
        handler_total_ns / (handler_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_other_ms = if handler_count > 0 {
        other_total_ns / (handler_count as f64) / 1_000_000.0
    } else {
        0.0
    };

    let calls = s.total_calls.load(Ordering::Relaxed);
    let hits = cache_hit_count;
    let hit_rate = if calls > 0 {
        (hits as f64) / (calls as f64) * 100.0
    } else {
        0.0
    };
    let cache_keys = s.cache_keys.load(Ordering::Relaxed);

    info!(
        "[DECODE BENCH] avg_api_ms={:.3} over {} api_calls | avg_cache_hit_ms={:.3} over {} hits | avg_decode_ms={:.3} over {} decodes| avg_decode_fail_ms={:.3} over {} decodes_fail | avg_handler_ms={:.3} over {} handlers | avg_other_ms={:.3} | cache_keys={} | calls={} | hit_rate={:.2}%",
        avg_api_ms,
        api_count,
        avg_cache_ms,
        cache_hit_count,
        avg_decode_ms,
        decode_count,
        avg_decode_fail_ms,
        decode_failed_count,
        avg_handler_ms,
        handler_count,
        avg_other_ms,
        cache_keys,
        calls,
        hit_rate
    );
}
