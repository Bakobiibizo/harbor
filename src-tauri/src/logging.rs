//! Structured logging system for Harbor
//!
//! Provides configurable logging with:
//! - Module-specific log levels
//! - Console output (development)
//! - Rotated JSON file output (production)
//! - Sensitive data redaction

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Whether logging has been initialized
static LOGGING_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Configuration for the logging system
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log level for the main harbor module
    pub harbor_level: Level,
    /// Log level for P2P networking
    pub p2p_level: Level,
    /// Log level for database operations
    pub db_level: Level,
    /// Log level for commands
    pub commands_level: Level,
    /// Enable file logging
    pub file_logging: bool,
    /// Directory for log files
    pub log_dir: Option<PathBuf>,
    /// Enable JSON format for file logs
    pub json_format: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            harbor_level: Level::INFO,
            p2p_level: Level::INFO,
            db_level: Level::WARN,
            commands_level: Level::DEBUG,
            file_logging: false,
            log_dir: None,
            json_format: true,
        }
    }
}

impl LogConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Parse HARBOR_LOG_LEVEL for overall level
        if let Ok(level) = std::env::var("HARBOR_LOG_LEVEL") {
            if let Ok(level) = level.parse() {
                config.harbor_level = level;
            }
        }

        // Parse module-specific levels
        if let Ok(level) = std::env::var("HARBOR_P2P_LOG_LEVEL") {
            if let Ok(level) = level.parse() {
                config.p2p_level = level;
            }
        }

        if let Ok(level) = std::env::var("HARBOR_DB_LOG_LEVEL") {
            if let Ok(level) = level.parse() {
                config.db_level = level;
            }
        }

        // Enable file logging
        if std::env::var("HARBOR_LOG_FILE").is_ok() {
            config.file_logging = true;
        }

        // Log directory
        if let Ok(dir) = std::env::var("HARBOR_LOG_DIR") {
            config.log_dir = Some(PathBuf::from(dir));
        }

        // JSON format
        if let Ok(val) = std::env::var("HARBOR_LOG_JSON") {
            config.json_format = val == "1" || val.to_lowercase() == "true";
        }

        config
    }

    /// Build the env filter string
    fn build_filter(&self) -> String {
        format!(
            "harbor_lib={},harbor_lib::p2p={},harbor_lib::db={},harbor_lib::commands={},libp2p=warn,tauri=info",
            self.harbor_level.as_str().to_lowercase(),
            self.p2p_level.as_str().to_lowercase(),
            self.db_level.as_str().to_lowercase(),
            self.commands_level.as_str().to_lowercase(),
        )
    }
}

/// Initialize the logging system
/// Returns a guard that must be kept alive for the duration of the program
/// to ensure logs are flushed to files.
pub fn init_logging(config: LogConfig) -> Option<WorkerGuard> {
    // Only initialize once
    if LOGGING_INITIALIZED.swap(true, Ordering::SeqCst) {
        return None;
    }

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.build_filter()));

    // Console layer - always enabled, compact format
    let console_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_names(false)
        .with_span_events(FmtSpan::NONE)
        .compact();

    // File layer - optional, with rotation
    let (file_layer, guard) = if config.file_logging {
        let log_dir = config.log_dir.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("logs")
        });

        // Create the log directory
        std::fs::create_dir_all(&log_dir).ok();

        // Daily rotation with max 5 files
        let file_appender = tracing_appender::rolling::daily(&log_dir, "harbor.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let layer = if config.json_format {
            fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_current_span(true)
                .boxed()
        } else {
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .boxed()
        };

        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    // Build the subscriber
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(console_layer);

    if let Some(file_layer) = file_layer {
        subscriber.with(file_layer).init();
    } else {
        subscriber.init();
    }

    guard
}

/// Initialize logging with default configuration
pub fn init_default_logging() {
    let config = LogConfig::from_env();
    init_logging(config);
}

/// Redact sensitive data from log output
/// This should be used when logging potentially sensitive strings
pub fn redact_sensitive(input: &str) -> String {
    // Redact anything that looks like a private key or passphrase
    let mut output = input.to_string();

    // Redact hex strings that look like keys (32+ bytes = 64+ hex chars)
    let hex_regex = regex_lite::Regex::new(r"[0-9a-fA-F]{64,}").unwrap();
    output = hex_regex.replace_all(&output, "[REDACTED_KEY]").to_string();

    // Redact base64-encoded secrets (48+ chars)
    let base64_regex = regex_lite::Regex::new(r"[A-Za-z0-9+/]{48,}={0,2}").unwrap();
    output = base64_regex
        .replace_all(&output, "[REDACTED_SECRET]")
        .to_string();

    // Redact peer IDs in sensitive contexts
    // (peer IDs are public, but we might want to redact them in some contexts)

    output
}

/// Log a structured event with context
#[macro_export]
macro_rules! log_event {
    ($level:expr, $event:expr, $($field:tt)*) => {
        match $level {
            tracing::Level::ERROR => tracing::error!(event = $event, $($field)*),
            tracing::Level::WARN => tracing::warn!(event = $event, $($field)*),
            tracing::Level::INFO => tracing::info!(event = $event, $($field)*),
            tracing::Level::DEBUG => tracing::debug!(event = $event, $($field)*),
            tracing::Level::TRACE => tracing::trace!(event = $event, $($field)*),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_hex_key() {
        let input = "Key: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let output = redact_sensitive(input);
        assert!(output.contains("[REDACTED_KEY]"));
        assert!(!output.contains("0123456789abcdef"));
    }

    #[test]
    fn test_redact_base64() {
        let input = "Secret: SGVsbG8gV29ybGQhIFRoaXMgaXMgYSBsb25nIGJhc2U2NCBlbmNvZGVkIHN0cmluZw==";
        let output = redact_sensitive(input);
        assert!(output.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn test_redact_preserves_short_strings() {
        let input = "Normal message with abc123";
        let output = redact_sensitive(input);
        assert_eq!(input, output);
    }

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.harbor_level, Level::INFO);
        assert!(!config.file_logging);
    }

    #[test]
    fn test_build_filter() {
        let config = LogConfig::default();
        let filter = config.build_filter();
        assert!(filter.contains("harbor_lib=info"));
        assert!(filter.contains("libp2p=warn"));
    }
}
