//! Secret and credential redaction for diagnostics and issue reports.

use regex::Regex;
use std::sync::OnceLock;

fn patterns() -> &'static Vec<(&'static str, Regex)> {
    static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            (
                "-----BEGIN [REDACTED] PRIVATE KEY-----\n[REDACTED]\n-----END PRIVATE KEY-----",
                Regex::new(r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----")
                    .unwrap(),
            ),
            (
                "[REDACTED]",
                Regex::new(r"\b(?:gh[pousr]_[A-Za-z0-9_]{12,}|github_pat_[A-Za-z0-9_]{16,})\b")
                    .unwrap(),
            ),
            (
                "[REDACTED]",
                Regex::new(r"\b(?:sk-ant-[A-Za-z0-9\-_]{10,}|sk-[A-Za-z0-9\-_]{16,}|AIza[0-9A-Za-z\-_]{20,}|xox[baprs]-[0-9A-Za-z\-]{10,})\b")
                    .unwrap(),
            ),
            (
                "Authorization: Bearer [REDACTED]",
                Regex::new(r"(?i)Authorization:\s*Bearer\s+[^\s\r\n]+").unwrap(),
            ),
            (
                "Authorization: Basic [REDACTED]",
                Regex::new(r"(?i)Authorization:\s*Basic\s+[^\s\r\n]+").unwrap(),
            ),
            (
                "$1[REDACTED]",
                Regex::new(r#"(?i)((?:"(?:api_key|apiKey|access_token|refresh_token|token|password|secret)"\s*:\s*")[^"]+")"#)
                    .unwrap(),
            ),
            (
                "$1[REDACTED]",
                Regex::new(r"(?m)((?:GITHUB_TOKEN|OPENAI_API_KEY|ANTHROPIC_API_KEY|API_KEY|ACCESS_TOKEN|PASSWORD)\s*=\s*)[^\s\r\n]+")
                    .unwrap(),
            ),
        ]
    })
}

pub(crate) fn redact_sensitive_text(input: &str) -> String {
    let mut out = input.to_owned();
    for (replacement, re) in patterns() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if !home.is_empty() {
            out = out.replace(&home, "~");
            let slash_home = home.replace('\\', "/");
            out = out.replace(&slash_home, "~");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive_text;

    #[test]
    fn redacts_tokens_headers_env_keys_and_pem_blocks() {
        let raw = "\
Authorization: Bearer ghp_AbCdEfGhIjKlMnOpQrStUvWxYz123456\n\
OPENAI_API_KEY=sk-proj-secret1234567890abcdef\n\
\"api_key\": \"sk-ant-api03-verysecretvalue\"\n\
-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----\n";
        let clean = redact_sensitive_text(raw);
        assert!(!clean.contains("ghp_"));
        assert!(!clean.contains("sk-proj-secret"));
        assert!(!clean.contains("sk-ant-api03"));
        assert!(!clean.contains("MIIEowIBAAKCAQEA"));
        assert!(clean.contains("[REDACTED]"));
    }
}
