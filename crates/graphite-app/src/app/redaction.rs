//! Credential redaction shared by the error log and the issue reporter.

use regex::Regex;
use std::sync::OnceLock;

struct Rule {
    pattern: Regex,
    replacement: &'static str,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        [
            // PEM private keys, whole blocks.
            (
                r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
                "-----BEGIN [REDACTED] PRIVATE KEY-----\n[REDACTED]\n-----END PRIVATE KEY-----",
            ),
            // `Authorization: Bearer …`, `Basic …`, `token …`.
            (
                r"(?i)\b(bearer|basic|token)(\s+)[A-Za-z0-9._~+/=\-]{8,}",
                "$1$2[REDACTED]",
            ),
            // Environment-style assignments: `OPENAI_API_KEY=…`, `GITHUB_TOKEN=…`.
            (
                r"\b([A-Z][A-Z0-9_]*(?:TOKEN|KEY|SECRET|PASSWORD|PASSWD)\s*=\s*)[^\s]+",
                "$1[REDACTED]",
            ),
            // Fields: `api_key=…`, `"x-api-key": "…"`, `password: …`, and similar.
            (
                r#"(?i)\b(x-api-key|x-goog-api-key|api[_-]?key|apikey|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|secret|password|passwd|pwd|token|key)(["']?\s*[:=]\s*["']?)[^\s"'&,;]+"#,
                "$1$2[REDACTED]",
            ),
            // `scheme://user:password@host`.
            (
                r"(?i)\b([a-z][a-z0-9+.\-]*://)[^/\s:@]+:[^/\s@]+@",
                "$1[REDACTED]@",
            ),
            // GitHub, OpenAI/Anthropic/OpenRouter, Google, GitLab, Slack, and AWS tokens.
            (
                r"\b(gh[opusr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|sk-[A-Za-z0-9_\-]+|AIza[0-9A-Za-z_\-]{20,}|glpat-[A-Za-z0-9_\-]{8,}|xox[abprs]-[A-Za-z0-9\-]{8,}|AKIA[0-9A-Z]{16})",
                "[REDACTED]",
            ),
        ]
        .into_iter()
        .map(|(pattern, replacement)| Rule {
            pattern: Regex::new(pattern).expect("valid redaction pattern"),
            replacement,
        })
        .collect()
    })
}

/// Replaces credentials in `input` with `[REDACTED]`: authorization schemes, `key=value`
/// secrets, passwords in URLs, PEM private keys, and the token formats of common services.
pub(crate) fn redact_credentials(input: &str) -> String {
    let mut text = input.to_owned();
    for rule in rules() {
        if let std::borrow::Cow::Owned(replaced) = rule.pattern.replace_all(&text, rule.replacement)
        {
            text = replaced;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::redact_credentials;

    #[test]
    fn every_supported_credential_format_is_redacted() {
        // Synthetic values in the shape of each service's credentials.
        let secrets = [
            (
                "GitHub OAuth token gho_fixture0123456789",
                "gho_fixture0123456789",
            ),
            ("classic ghp_fixture0123456789", "ghp_fixture0123456789"),
            ("user token ghu_fixture0123456789", "ghu_fixture0123456789"),
            (
                "server token ghs_fixture0123456789",
                "ghs_fixture0123456789",
            ),
            ("fine-grained github_pat_fixture", "github_pat_fixture"),
            ("Anthropic sk-ant-fixture-0123", "sk-ant-fixture-0123"),
            ("OpenRouter sk-or-v1-fixture0123", "sk-or-v1-fixture0123"),
            (
                "Gemini AIzaFixtureFixtureFixture012345",
                "AIzaFixtureFixtureFixture012345",
            ),
            ("GitLab glpat-fixture0123456789", "glpat-fixture0123456789"),
            ("Slack xoxb-fixture-0123456789", "xoxb-fixture-0123456789"),
            ("AWS AKIAFIXTUREFIXTURE01", "AKIAFIXTUREFIXTURE01"),
            (
                "Authorization: Bearer fixture.token.value",
                "fixture.token.value",
            ),
            (
                "Authorization: Basic Zml4dHVyZTpmaXh0dXJl",
                "Zml4dHVyZTpmaXh0dXJl",
            ),
            ("header x-api-key: fixturevalue01", "fixturevalue01"),
            ("query ?api_key=fixturevalue02&model=m", "fixturevalue02"),
            (r#"body {"password": "fixturevalue03"}"#, "fixturevalue03"),
            ("env OPENAI_API_KEY=fixturevalue04", "fixturevalue04"),
            ("env GITHUB_TOKEN=fixturevalue05", "fixturevalue05"),
            (
                "url https://fixtureuser:fixturepass@example.invalid/v1",
                "fixturepass",
            ),
        ];
        for (raw, secret) in secrets {
            let clean = redact_credentials(raw);
            assert!(!clean.contains(secret), "{raw} -> {clean}");
            assert!(clean.contains("[REDACTED]"), "{raw} -> {clean}");
        }
    }

    #[test]
    fn private_key_blocks_are_removed_whole() {
        let raw = "-----BEGIN RSA PRIVATE KEY-----\nFIXTUREKEYBODY\n-----END RSA PRIVATE KEY-----";
        let clean = redact_credentials(raw);
        assert!(!clean.contains("FIXTUREKEYBODY"));
    }

    #[test]
    fn ordinary_text_is_kept() {
        for raw in [
            "Could not open D:/work/notes.md: access denied (os error 5)",
            "request to https://example.invalid/v1/chat failed with status 404",
            "The model service rejected the API key",
            "Missing keyboard layout for the desk-top view",
        ] {
            assert_eq!(redact_credentials(raw), raw);
        }
    }
}
