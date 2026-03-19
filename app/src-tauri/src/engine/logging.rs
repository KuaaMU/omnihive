//! Structured logging initialization using the `tracing` crate.

use std::path::Path;

/// Initialize structured logging.
///
/// Sets up a layered subscriber:
/// - JSON file appender for machine-readable logs (if `log_dir` provided)
/// - Stderr output for human-readable logs
pub fn init_logging(log_dir: Option<&Path>) {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{fmt, EnvFilter};

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("omnihive=info"));

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact();

    if let Some(dir) = log_dir {
        let file_appender = tracing_appender::rolling::daily(dir, "omnihive.log");
        let json_layer = fmt::layer()
            .json()
            .with_writer(file_appender)
            .with_target(true)
            .with_span_events(fmt::format::FmtSpan::CLOSE);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(json_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .init();
    }
}
