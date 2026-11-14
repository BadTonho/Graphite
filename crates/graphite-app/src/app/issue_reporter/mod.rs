//! Redacted issue reporter and interactive troubleshooting session tracking.

pub(crate) mod redact;

use crate::App;
use redact::redact_sensitive_text;

#[derive(Debug, Clone, Default)]
pub(crate) struct TroubleshootingSession {
    pub active: bool,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueReportDraft {
    pub title: String,
    pub body_markdown: String,
    pub github_url: String,
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl App {
    pub(crate) fn collect_redacted_diagnostics(&self, summary: Option<&str>) -> String {
        let workspace = self.tree.root().display().to_string();
        let remote_alias = self
            .remote
            .active_host
            .as_ref()
            .map(|h| h.alias.as_str())
            .unwrap_or("local");
        let extension_count = self.plugins.installed.len();
        let mut report = format!(
            "## Summary\n\n{}\n\n## Environment\n\n- Graphite Version: `{}`\n- OS / Arch: `{}` / `{}`\n- Update Channel: `{}`\n- Workspace: `{}`\n- Open Documents: `{}`\n- Installed Extensions: `{}`\n- Remote Context: `{}`\n- Troubleshooting Active: `{}`\n",
            summary.unwrap_or("_Describe the unexpected behavior and steps to reproduce._"),
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.updates.config.channel.label(),
            workspace,
            self.documents.len(),
            extension_count,
            remote_alias,
            self.troubleshooting.active,
        );
        if !self.troubleshooting.events.is_empty() {
            report.push_str("\n## Troubleshooting Log\n\n");
            for line in self.troubleshooting.events.iter().rev().take(25).rev() {
                report.push_str(&format!("- {line}\n"));
            }
        }
        redact_sensitive_text(&report)
    }

    pub(crate) fn build_issue_report_draft(&self, summary: Option<&str>) -> IssueReportDraft {
        let title = redact_sensitive_text(summary.unwrap_or("Bug report from Graphite"));
        let body_markdown = self.collect_redacted_diagnostics(summary);
        let repo = &self.updates.config.repo;
        let github_url = format!(
            "https://github.com/{repo}/issues/new?title={}&body={}",
            percent_encode(&title),
            percent_encode(&body_markdown)
        );
        IssueReportDraft {
            title,
            body_markdown,
            github_url,
        }
    }

    pub(crate) fn open_issue_reporter(&mut self, summary: Option<&str>) -> IssueReportDraft {
        let draft = self.build_issue_report_draft(summary);
        self.new_untitled_document();
        let content = format!(
            "# Graphite Issue Reporter (Secrets Redacted)\n\nGitHub Issue URL:\n`{}`\n\n---\n\n{}",
            draft.github_url, draft.body_markdown
        );
        self.editor_mut().insert(&content);
        self.status = "Opened redacted issue report draft".into();
        draft
    }

    pub(crate) fn start_troubleshooting(&mut self) {
        self.troubleshooting.active = true;
        let msg = format!(
            "Started troubleshooting session (extensions installed: {}, remote: {:?})",
            self.plugins.installed.len(),
            self.remote.active_host.as_ref().map(|h| h.alias.as_str())
        );
        self.troubleshooting.events.push(msg);
        self.status = "Troubleshooting session started".into();
    }

    pub(crate) fn stop_troubleshooting(&mut self) {
        if !self.troubleshooting.active {
            self.status = "No active troubleshooting session".into();
            return;
        }
        self.troubleshooting.active = false;
        self.troubleshooting
            .events
            .push("Stopped troubleshooting session".into());
        self.open_issue_reporter(Some("Troubleshooting session summary"));
        self.status = "Stopped troubleshooting session and opened report".into();
    }
}
