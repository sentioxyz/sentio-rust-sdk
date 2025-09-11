use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::info;

struct BenchState {
    api_total_ns: u128,
    api_count: u64,
    cache_hit_total_ns: u128,
    cache_hit_count: u64,
    decode_total_ns: u128,
    decode_count: u64,
    decode_failed_total_ns: u128,
    decode_failed_count: u64,
    handler_total_ns: u128,
    handler_count: u64,
    other_total_ns: u128,
    total_calls: u64,
    cache_keys: u64,
    last_print: Instant,
}

impl BenchState {
    fn new() -> Self {
        Self {
            api_total_ns: 0,
            api_count: 0,
            cache_hit_total_ns: 0,
            cache_hit_count: 0,
            decode_failed_total_ns:0,
            decode_failed_count: 0,
            decode_total_ns: 0,
            decode_count: 0,
            handler_total_ns: 0,
            handler_count: 0,
            other_total_ns: 0,
            total_calls: 0,
            cache_keys: 0,
            last_print: Instant::now(),
        }
    }
}

static ENABLED: OnceLock<bool> = OnceLock::new();
fn enabled() -> bool {
    *ENABLED
        .get_or_init(|| std::env::var("SHOW_BENCHMARK_RESULT").is_ok())
}

static STATE: OnceLock<Mutex<BenchState>> = OnceLock::new();
fn state() -> &'static Mutex<BenchState> {
    STATE.get_or_init(|| Mutex::new(BenchState::new()))
}

pub fn record_api_call(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.api_total_ns += dur.as_nanos();
    s.api_count += 1;
}

pub fn record_cache_hit(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.cache_hit_total_ns += dur.as_nanos();
    s.cache_hit_count += 1;
}

pub fn record_decode(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.decode_total_ns += dur.as_nanos();
    s.decode_count += 1;
}

pub fn record_decode_failed(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.decode_failed_total_ns += dur.as_nanos();
    s.decode_failed_count += 1;
}


pub fn record_handler(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.handler_total_ns += dur.as_nanos();
    s.handler_count += 1;
}

pub fn record_other(dur: Duration) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.other_total_ns += dur.as_nanos();
}

pub fn mark_call() {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.total_calls += 1;
}

pub fn update_cache_size(keys: u64) {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    s.cache_keys = keys;
}

pub fn print_if_due() {
    if !enabled() {
        return;
    }
    let mut s = state().lock().unwrap();
    if s.last_print.elapsed().as_secs() < 60 {
        return;
    }
    let avg_api_ms = if s.api_count > 0 {
        (s.api_total_ns as f64) / (s.api_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_cache_ms = if s.cache_hit_count > 0 {
        (s.cache_hit_total_ns as f64) / (s.cache_hit_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_decode_ms = if s.decode_count > 0 {
        (s.decode_total_ns as f64) / (s.decode_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_decode_fail_ms = if s.decode_failed_count > 0 {
        (s.decode_failed_total_ns as f64) / (s.decode_failed_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_handler_ms = if s.handler_count > 0 {
        (s.handler_total_ns as f64) / (s.handler_count as f64) / 1_000_000.0
    } else {
        0.0
    };
    let avg_other_ms = if s.handler_count > 0 {
        (s.other_total_ns as f64) / (s.handler_count as f64) / 1_000_000.0
    } else {
        0.0
    };

    let calls = s.total_calls;
    let hits = s.cache_hit_count;
    let hit_rate = if calls > 0 { (hits as f64) / (calls as f64) * 100.0 } else { 0.0 };

    info!(
        "[DECODE BENCH] avg_api_ms={:.3} over {} api_calls | avg_cache_hit_ms={:.3} over {} hits | avg_decode_ms={:.3} over {} decodes| avg_decode_fail_ms={:.3} over {} decodes_fail | avg_handler_ms={:.3} over {} handlers | avg_other_ms={:.3} | cache_keys={} | calls={} | hit_rate={:.2}%",
        avg_api_ms,
        s.api_count,
        avg_cache_ms,
        s.cache_hit_count,
        avg_decode_ms,
        s.decode_count,
        avg_decode_fail_ms,
        s.decode_count,
        avg_handler_ms,
        s.handler_count,
        avg_other_ms,
        s.cache_keys,
        calls,
        hit_rate
    );

    s.last_print = Instant::now();

    // Reset counters after printing (windowed averages)
    s.api_total_ns = 0;
    s.api_count = 0;
    s.decode_total_ns = 0;
    s.decode_count = 0;
    s.handler_total_ns = 0;
    s.handler_count = 0;
    s.other_total_ns = 0;
 }
