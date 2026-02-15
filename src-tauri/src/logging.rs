use std::path::PathBuf;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct LogConfig {
    pub log_dir: Option<PathBuf>,
    pub console_enabled: bool,
    pub file_enabled: bool,
    pub json_format: bool,
    pub max_files: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_dir: None,
            console_enabled: true,
            file_enabled: false,
            json_format: false,
            max_files: 5,
        }
    }
}

impl LogConfig {
    pub fn development() -> Self {
        Self {
            console_enabled: true,
            file_enabled: false,
            json_format: false,
            ..Default::default()
        }
    }

    pub fn production(log_dir: PathBuf) -> Self {
        Self {
            log_dir: Some(log_dir),
            console_enabled: false,
            file_enabled: true,
            json_format: true,
            max_files: 5,
        }
    }
}

fn get_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "harbor_lib=debug,\
             harbor_lib::p2p=debug,\
             harbor_lib::db=info,\
             harbor_lib::services=debug,\
             harbor_lib::commands=debug,\
             tauri=info,\
             libp2p=warn",
        )
    })
}

pub fn init_logging(config: LogConfig) {
    let env_filter = get_env_filter();

    let registry = tracing_subscriber::registry();

    if config.console_enabled {
        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(env_filter.clone());

        if let Some(log_dir) = config.log_dir.filter(|_| config.file_enabled) {
            std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

            let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "harbor.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

            LOG_GUARD.set(guard).ok();

            if config.json_format {
                let file_layer = fmt::layer()
                    .json()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_filter(get_env_filter());

                registry.with(console_layer).with(file_layer).init();
            } else {
                let file_layer = fmt::layer()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_ansi(false)
                    .with_filter(get_env_filter());

                registry.with(console_layer).with(file_layer).init();
            }
        } else {
            registry.with(console_layer).init();
        }
    } else if let Some(log_dir) = config.log_dir.filter(|_| config.file_enabled) {
        std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

        let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "harbor.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        LOG_GUARD.set(guard).ok();

        if config.json_format {
            let file_layer = fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(env_filter);

            registry.with(file_layer).init();
        } else {
            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_target(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_filter(env_filter);

            registry.with(file_layer).init();
        }
    } else {
        let noop_layer = fmt::layer()
            .with_writer(std::io::sink)
            .with_filter(env_filter);
        registry.with(noop_layer).init();
    }
}

pub fn get_log_directory(app_data_dir: &std::path::Path) -> PathBuf {
    app_data_dir.join("logs")
}

pub fn export_logs(log_dir: &std::path::Path) -> Result<String, std::io::Error> {
    let mut logs = String::new();
    logs.push_str("=== Harbor Log Export ===\n");
    logs.push_str(&format!("Export Time: {}\n", chrono::Utc::now()));
    logs.push_str(&format!("OS: {}\n", std::env::consts::OS));
    logs.push_str(&format!("Arch: {}\n", std::env::consts::ARCH));
    logs.push_str("========================\n\n");

    if log_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(log_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "log")
                    .unwrap_or(false)
            })
            .collect();

        entries.sort_by_key(|e| e.path());

        for entry in entries.iter().rev().take(3) {
            let path = entry.path();
            logs.push_str(&format!("\n--- {} ---\n", path.display()));

            let content = std::fs::read_to_string(&path)?;
            let redacted = redact_sensitive_data(&content);

            let lines: Vec<&str> = redacted.lines().collect();
            let start = if lines.len() > 1000 {
                lines.len() - 1000
            } else {
                0
            };
            for line in &lines[start..] {
                logs.push_str(line);
                logs.push('\n');
            }
        }
    } else {
        logs.push_str("No log files found.\n");
    }

    Ok(logs)
}

fn redact_sensitive_data(content: &str) -> String {
    let mut redacted = content.to_string();

    let patterns = [
        (r"passphrase[^,}\]]*", "passphrase: [REDACTED]"),
        (r"private_key[^,}\]]*", "private_key: [REDACTED]"),
        (r"secret[^,}\]]*", "secret: [REDACTED]"),
        (r"password[^,}\]]*", "password: [REDACTED]"),
        (r"encryption_key[^,}\]]*", "encryption_key: [REDACTED]"),
    ];

    for (pattern, replacement) in patterns {
        if let Ok(re) = regex::Regex::new(&format!("(?i){}", pattern)) {
            redacted = re.replace_all(&redacted, replacement).to_string();
        }
    }

    redacted
}

pub fn cleanup_old_logs(log_dir: &std::path::Path, max_files: usize) -> Result<(), std::io::Error> {
    if !log_dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(log_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "log")
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    if entries.len() > max_files {
        let to_remove = entries.len() - max_files;
        for entry in entries.into_iter().take(to_remove) {
            std::fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}
