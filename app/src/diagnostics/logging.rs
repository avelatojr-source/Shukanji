//! Structured logging setup.
//!
//! Logs are written to a rolling file under the diagnostics directory.
//! Secrets must only be logged via `diagnostics::redact` helpers.

use std::path::Path;

use tracing_subscriber::EnvFilter;

/// Guard that keeps the background log writer alive for the program lifetime.
pub struct LogGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Initialize logging. Returns a guard that must be kept alive.
///
/// `log_dir` is created if missing. If the file writer cannot be created we
/// fall back to a sink rather than failing startup.
pub fn init(log_dir: Option<&Path>) -> LogGuard {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tiktok_stream_tool=debug"));

    if let Some(dir) = log_dir
        && std::fs::create_dir_all(dir).is_ok()
    {
        let appender = tracing_appender::rolling::daily(dir, "tiktok-stream-tool.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .with_ansi(false)
            .finish();
        // Ignore "already set" errors so tests can call init harmlessly.
        let _ = tracing::subscriber::set_global_default(subscriber);
        return LogGuard { _file_guard: guard };
    }

    // Fallback: discard output but keep a guard of the same type.
    let (writer, guard) = tracing_appender::non_blocking(std::io::sink());
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    LogGuard { _file_guard: guard }
}
