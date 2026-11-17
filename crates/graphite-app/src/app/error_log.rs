//! Four-tier error persistence for Graphite (`critical`, `high`, `medium`, `low`).

use regex::Regex;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Four severity levels ordered from highest (`Critical`) to lowest (`Low`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ErrorSeverity {
    /// Fatal crash, unhandled panic, or unrecoverable startup/GPU/window failure.
    Critical,
    /// Core workflow failure (file IO, corrupted config, update apply/verify, host crash).
    High,
    /// Subsystem degradation where the editor continues running (LSP, Git, terminal, plugins).
    Medium,
    /// Non-critical background/UI failure or minor stall.
    Low,
}

impl ErrorSeverity {
    pub(crate) const fn file_name(self) -> &'static str {
        match self {
            Self::Critical => "critical.jsonl",
            Self::High => "high.jsonl",
            Self::Medium => "medium.jsonl",
            Self::Low => "low.jsonl",
        }
    }

    pub(crate) const fn rotated_file_name(self) -> &'static str {
        match self {
            Self::Critical => "critical.1.jsonl",
            Self::High => "high.1.jsonl",
            Self::Medium => "medium.1.jsonl",
            Self::Low => "low.1.jsonl",
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorLogEntry<'a> {
    timestamp_unix_ms: u64,
    severity: ErrorSeverity,
    source: &'a str,
    message: String,
    os: &'static str,
    version: &'static str,
}

pub(crate) struct ErrorLogger {
    log_dir: PathBuf,
    write_lock: Mutex<()>,
}

static GLOBAL_LOGGER: OnceLock<ErrorLogger> = OnceLock::new();
static PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();
static EVENT_PROXY: OnceLock<
    winit::event_loop::EventLoopProxy<crate::app::accessibility::UserEvent>,
> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErrorNotificationItem {
    pub severity: ErrorSeverity,
    pub source: String,
    pub message: String,
    pub timestamp_unix_ms: u64,
    pub log_file: PathBuf,
}

impl ErrorLogger {
    pub(crate) fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            write_lock: Mutex::new(()),
        }
    }

    /// Writes one sanitized JSON line into `<log_dir>/<severity>.jsonl`, rotating at 5 MB.
    pub(crate) fn record(
        &self,
        severity: ErrorSeverity,
        source: &str,
        message: &str,
    ) -> ErrorNotificationItem {
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let sanitized = sanitize_message(message);
        let target = self.log_dir.join(severity.file_name());
        let notification = ErrorNotificationItem {
            severity,
            source: source.to_owned(),
            message: sanitized.clone(),
            timestamp_unix_ms,
            log_file: target.clone(),
        };
        let entry = ErrorLogEntry {
            timestamp_unix_ms,
            severity,
            source,
            message: sanitized,
            os: std::env::consts::OS,
            version: env!("CARGO_PKG_VERSION"),
        };
        let Ok(mut line) = serde_json::to_string(&entry) else {
            return notification;
        };
        line.push('\n');

        let Ok(_guard) = self.write_lock.lock() else {
            return notification;
        };
        if fs::create_dir_all(&self.log_dir).is_err() {
            return notification;
        }
        rotate_if_oversized(
            &target,
            &self.log_dir.join(severity.rotated_file_name()),
            MAX_LOG_BYTES,
        );
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&target) {
            let _ = file.write_all(line.as_bytes());
        }
        notification
    }
}

fn rotate_if_oversized(target: &Path, backup: &Path, max_bytes: u64) {
    let Ok(meta) = fs::metadata(target) else {
        return;
    };
    if meta.len() >= max_bytes {
        let _ = fs::remove_file(backup);
        let _ = fs::rename(target, backup);
    }
}

/// Redacts common bearer tokens and API key prefixes before persisting log lines.
pub(crate) fn sanitize_message(raw: &str) -> String {
    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    let re = TOKEN_RE.get_or_init(|| {
        Regex::new(r"(?i)(bearer\s+[A-Za-z0-9._\-]+|ghp_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|sk-[A-Za-z0-9_\-]+)")
            .expect("valid sanitization regex")
    });
    re.replace_all(raw, "[REDACTED]").into_owned()
}

/// Initializes the global error logger and installs a panic hook for `Critical` crashes.
pub(crate) fn init(log_dir: PathBuf) {
    let _ = GLOBAL_LOGGER.set(ErrorLogger::new(log_dir));
    PANIC_HOOK_INSTALLED.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_owned()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "non-string panic payload".to_owned()
            };
            let location = info
                .location()
                .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
                .unwrap_or_else(|| "unknown location".to_owned());
            record(
                ErrorSeverity::Critical,
                "panic",
                format!("panic at {location}: {payload}"),
            );
            previous(info);
        }));
    });
}

/// Registers the event-loop proxy so recorded errors surface as UI notifications.
pub(crate) fn set_event_proxy(
    proxy: winit::event_loop::EventLoopProxy<crate::app::accessibility::UserEvent>,
) {
    let _ = EVENT_PROXY.set(proxy);
}

fn should_notify_ui(severity: ErrorSeverity, source: &str) -> bool {
    !(severity == ErrorSeverity::Low && source == "updates.background_check")
}

/// Records an error into the global four-tier error log if initialized.
pub(crate) fn record(severity: ErrorSeverity, source: &str, message: impl AsRef<str>) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let item = logger.record(severity, source, message.as_ref());
        if should_notify_ui(severity, source) {
            if let Some(proxy) = EVENT_PROXY.get() {
                let _ = proxy.send_event(crate::app::accessibility::UserEvent::ErrorRecorded(item));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_each_severity_to_its_own_jsonl_file() {
        let dir = crate::test_support::TempDir::new();
        let logger = ErrorLogger::new(dir.path().to_path_buf());

        logger.record(ErrorSeverity::Critical, "panic", "fatal gpu crash");
        logger.record(ErrorSeverity::High, "file_io", "failed saving document");
        logger.record(ErrorSeverity::Medium, "lsp", "rust-analyzer exited");
        logger.record(ErrorSeverity::Low, "updates", "background check offline");

        for (severity, expected_source) in [
            (ErrorSeverity::Critical, "panic"),
            (ErrorSeverity::High, "file_io"),
            (ErrorSeverity::Medium, "lsp"),
            (ErrorSeverity::Low, "updates"),
        ] {
            let path = dir.path().join(severity.file_name());
            let content = fs::read_to_string(&path).expect("severity log file exists");
            let parsed: serde_json::Value =
                serde_json::from_str(content.trim()).expect("valid json line");
            assert_eq!(parsed["source"], expected_source);
            assert_eq!(parsed["version"], env!("CARGO_PKG_VERSION"));
        }
    }

    #[test]
    fn sanitize_message_redacts_common_secret_tokens() {
        let raw = "auth failed with Bearer abc.def-123 and ghp_SecretToken999 and sk-proj12345";
        let sanitized = sanitize_message(raw);
        assert!(!sanitized.contains("abc.def-123"));
        assert!(!sanitized.contains("ghp_SecretToken999"));
        assert!(!sanitized.contains("sk-proj12345"));
        assert_eq!(sanitized.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn rotates_oversized_log_file_and_keeps_writing_valid_jsonl() {
        let dir = crate::test_support::TempDir::new();
        let target = dir.path().join(ErrorSeverity::High.file_name());
        let backup = dir.path().join(ErrorSeverity::High.rotated_file_name());
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(&target, vec![b'x'; 64]).unwrap();

        rotate_if_oversized(&target, &backup, 32);
        assert!(backup.exists());
        assert!(!target.exists());

        let logger = ErrorLogger::new(dir.path().to_path_buf());
        logger.record(ErrorSeverity::High, "updates", "sha256 mismatch");
        let fresh = fs::read_to_string(&target).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(fresh.trim()).unwrap();
        assert_eq!(parsed["message"], "sha256 mismatch");
    }

    #[test]
    fn concurrent_writes_preserve_valid_json_lines() {
        let dir = crate::test_support::TempDir::new();
        let logger = std::sync::Arc::new(ErrorLogger::new(dir.path().to_path_buf()));

        std::thread::scope(|scope| {
            for worker in 0..4 {
                let logger = logger.clone();
                scope.spawn(move || {
                    for i in 0..10 {
                        logger.record(
                            ErrorSeverity::Medium,
                            "worker",
                            &format!("thread {worker} event {i}"),
                        );
                    }
                });
            }
        });

        let content =
            fs::read_to_string(dir.path().join(ErrorSeverity::Medium.file_name())).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 40);
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed["severity"], "medium");
        }
    }
}
