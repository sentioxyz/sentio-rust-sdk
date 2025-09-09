//! Performance profiling utilities with HTTP endpoint support
//! 
//! This module provides CPU profiling capabilities using pprof, with an integrated
//! HTTP server for on-demand profiling and zip package generation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use anyhow::Result;
use axum::extract::Query;
use axum::http::{Response, StatusCode};
use axum::response::Html;
use tracing::{info, error};

#[cfg(feature = "profiling")]
use pprof::ProfilerGuard;

/// Global profiler state management
static PROFILER_STATE: std::sync::OnceLock<Arc<Mutex<ProfilerState>>> = std::sync::OnceLock::new();

struct ProfilerState {
    #[cfg(feature = "profiling")]
    guard: Option<ProfilerGuard<'static>>,
    #[cfg(not(feature = "profiling"))]
    _phantom: std::marker::PhantomData<()>,
    start_time: Option<SystemTime>,
    is_running: bool,
}

impl Default for ProfilerState {
    fn default() -> Self {
        Self {
            #[cfg(feature = "profiling")]
            guard: None,
            #[cfg(not(feature = "profiling"))]
            _phantom: std::marker::PhantomData,
            start_time: None,
            is_running: false,
        }
    }
}

/// Performance profiler with HTTP endpoint integration
pub struct Profiler {
    /// Profiling frequency in Hz
    frequency: i32,
    /// Port for HTTP profiling server (default: 4040)
    http_port: u16,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// Create a new profiler with default settings (port 4040)
    pub fn new() -> Self {
        Self {
            frequency: 1000, // 1000Hz sampling frequency
            http_port: 4040, // Default port 4040
        }
    }

    /// Create a profiler with custom frequency
    pub fn with_frequency(frequency: i32) -> Self {
        Self {
            frequency,
            http_port: 4040,
        }
    }

    /// Set custom HTTP profiling endpoint port
    pub fn with_http_endpoint(mut self, port: u16) -> Self {
        self.http_port = port;
        self
    }

    /// Start the profiler
    pub fn start(&self) -> Result<()> {
        let state = PROFILER_STATE.get_or_init(|| Arc::new(Mutex::new(ProfilerState::default())));
        let mut state = state.lock().unwrap();

        if state.is_running {
            return Err(anyhow::anyhow!("Profiler is already running"));
        }

        #[cfg(feature = "profiling")]
        {
            let guard = pprof::ProfilerGuardBuilder::default()
                .frequency(self.frequency)
                .blocklist(&["libc", "libgcc", "pthread", "vdso"])
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to start profiler: {}", e))?;

            state.guard = Some(guard);
            state.start_time = Some(SystemTime::now());
            state.is_running = true;
            
            info!("🔥 CPU profiler started with {}Hz sampling frequency", self.frequency);
        }

        #[cfg(not(feature = "profiling"))]
        {
            return Err(anyhow::anyhow!("Profiling feature not enabled"));
        }

        Ok(())
    }

    /// Stop profiling and generate flamegraph
    pub fn stop_and_generate_flamegraph(&self) -> Result<String> {
        let state = PROFILER_STATE.get_or_init(|| Arc::new(Mutex::new(ProfilerState::default())));
        let mut state = state.lock().unwrap();

        if !state.is_running {
            return Err(anyhow::anyhow!("Profiler is not running"));
        }

        let actual_duration = state.start_time
            .map(|start| SystemTime::now().duration_since(start).unwrap_or_default())
            .unwrap_or_default();

        #[cfg(feature = "profiling")]
        {
            if let Some(profiler) = state.guard.take() {
                match profiler.report().build() {
                    Ok(report) => {
                        let mut flamegraph_buffer = Vec::new();
                        report.flamegraph(&mut flamegraph_buffer)
                            .map_err(|e| anyhow::anyhow!("Failed to generate flamegraph: {}", e))?;

                        state.is_running = false;
                        state.start_time = None;

                        info!("🔥 Profiler stopped. Generated flamegraph (duration: {:.2}s)", 
                              actual_duration.as_secs_f64());
                        
                        Ok(String::from_utf8_lossy(&flamegraph_buffer).to_string())
                    }
                    Err(e) => {
                        error!("Failed to generate profiler report: {}", e);
                        state.is_running = false;
                        state.start_time = None;
                        Err(anyhow::anyhow!("Failed to generate profiler report: {}", e))
                    }
                }
            } else {
                Err(anyhow::anyhow!("No active profiler guard found"))
            }
        }

        #[cfg(not(feature = "profiling"))]
        {
            Err(anyhow::anyhow!("Profiling feature not enabled"))
        }
    }

    /// Check if profiler is currently running
    pub fn is_running(&self) -> bool {
        PROFILER_STATE
            .get()
            .map(|state| state.lock().unwrap().is_running)
            .unwrap_or(false)
    }

    /// Get profiler status and duration
    pub fn status(&self) -> ProfilerStatus {
        let state = PROFILER_STATE.get_or_init(|| Arc::new(Mutex::new(ProfilerState::default())));
        let state = state.lock().unwrap();
        
        let duration = if state.is_running {
            state.start_time
                .map(|start| SystemTime::now().duration_since(start).unwrap_or_default())
                .unwrap_or_default()
        } else {
            Duration::from_secs(0)
        };

        ProfilerStatus {
            is_running: state.is_running,
            duration,
            frequency: self.frequency,
        }
    }

    /// Start HTTP profiling server
    #[cfg(feature = "profiling")]
    pub async fn start_http_server(&self) -> Result<()> {
        use axum::{
            routing::get,
            Router,
        };

        let port = self.http_port;
        
        let app = Router::new()
            .route("/profile", get(profile_handler));

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind profiling server: {}", e))?;

        info!("Profiling HTTP server started on http://0.0.0.0:{}", port);


        axum::serve(listener, app)
            .await
            .map_err(|e| anyhow::anyhow!("Profiling server error: {}", e))?;

        Ok(())
    }

    #[cfg(not(feature = "profiling"))]
    pub async fn start_http_server(&self) -> Result<()> {
        Err(anyhow::anyhow!("HTTP profiling server requires profiling feature"))
    }
}

/// Profiler status information
#[derive(Debug, Clone)]
pub struct ProfilerStatus {
    pub is_running: bool,
    pub duration: Duration,
    pub frequency: i32,
}

// HTTP Handlers for profiling endpoints
#[cfg(feature = "profiling")]
async fn profile_handler(Query(params): Query<HashMap<String, String>>) -> Result<Response<axum::body::Body>, StatusCode> {
    use axum::response::IntoResponse;
    use axum::http::header;

    // Parse duration parameter (default 10 seconds)
    let duration: u64 = params
        .get("t")
        .and_then(|d| d.parse().ok())
        .unwrap_or(10);

    if duration > 300 { // Max 5 minutes
        let error_html = format!(
            "<html><body><h1>Invalid Duration</h1><p>Maximum profiling duration is 300 seconds. Requested: {}s</p></body></html>", 
            duration
        );
        return Ok((StatusCode::BAD_REQUEST, Html(error_html)).into_response());
    }

    let profiler = Profiler::new();
    
    // Start profiling
    if let Err(e) = profiler.start() {
        let error_html = format!(
            "<html><body><h1>Profiling Error</h1><p>Failed to start profiler: {}</p></body></html>", 
            e
        );
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, Html(error_html)).into_response());
    }

    // Wait for specified duration
    tokio::time::sleep(Duration::from_secs(duration)).await;

    // Stop and generate flamegraph
    match profiler.stop_and_generate_flamegraph() {
        Ok(flamegraph_svg) => {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/svg+xml")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"sentio_profile_{}s.svg\"", duration)
                )
                .body(axum::body::Body::from(flamegraph_svg))
                .unwrap();
            
            Ok(response)
        }
        Err(e) => {
            let error_html = format!(
                "<html><body><h1>Profiling Error</h1><p>Failed to generate profiling results: {}</p></body></html>", 
                e
            );
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Html(error_html)).into_response())
        }
    }
}


/// Global profiler instance for easy access
pub static GLOBAL_PROFILER: std::sync::OnceLock<Profiler> = std::sync::OnceLock::new();

/// Initialize global profiler with port 4040
pub fn init_global_profiler() -> &'static Profiler {
    GLOBAL_PROFILER.get_or_init(|| {
        Profiler::with_frequency(1000)
            .with_http_endpoint(4040) // Default port 4040
    })
}

/// Start profiling using global profiler
pub fn start_profiling() -> Result<()> {
    init_global_profiler().start()
}

/// Stop profiling and generate flamegraph using global profiler  
pub fn stop_profiling_flamegraph() -> Result<String> {
    init_global_profiler().stop_and_generate_flamegraph()
}

/// Get profiling status using global profiler
pub fn profiling_status() -> ProfilerStatus {
    init_global_profiler().status()
}

/// Start HTTP profiling server using global profiler on port 4040
pub async fn start_profiling_server() -> Result<()> {
    init_global_profiler().start_http_server().await
}