//! Secret and credential redaction for diagnostics and issue reports.

/// Redacts credentials and replaces the user's home folder with `~`.
pub(crate) fn redact_sensitive_text(input: &str) -> String {
    let mut out = crate::app::redaction::redact_credentials(input);
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
