//! Shell integration: reads the OSC 133 (prompt marks, exit codes) and OSC 7 (working
//! directory) sequences that shells print, and prepares the shells that Graphite starts so they
//! print them.

/// Longest OSC payload kept while waiting for its terminator; longer ones are dropped.
const MAX_PAYLOAD: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// The shell is about to draw a prompt (`OSC 133;A`).
    PromptStart,
    /// The previous command finished with this exit code (`OSC 133;D;code`).
    CommandEnd(Option<i32>),
    /// The working directory changed (`OSC 7;file://host/path`), as a local path.
    Cwd(String),
}

/// Finds shell integration sequences in a byte stream that may be split across reads.
#[derive(Debug, Default)]
pub struct ShellScanner {
    /// Payload of an OSC sequence that has started but not ended yet.
    pending: Option<Vec<u8>>,
    /// Offset of the ESC that started the pending sequence.
    pending_start: usize,
    /// Whether the last byte seen was ESC, so `ESC \` (ST) can end an OSC.
    after_escape: bool,
    /// Bytes consumed so far, across calls.
    offset: usize,
}

impl ShellScanner {
    /// Returns each event with the byte offset (in this call's `bytes`) where its sequence starts.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<(usize, ShellEvent)> {
        let mut events = Vec::new();
        for (index, &byte) in bytes.iter().enumerate() {
            let start = self.offset + index;
            if let Some(payload) = self.pending.as_mut() {
                match byte {
                    0x07 => self.finish(&mut events),
                    0x1b => self.after_escape = true,
                    // `ESC \` is the string terminator (ST).
                    b'\\' if self.after_escape => self.finish(&mut events),
                    _ => {
                        if self.after_escape {
                            self.after_escape = false;
                            payload.push(0x1b);
                        }
                        if payload.len() < MAX_PAYLOAD {
                            payload.push(byte);
                        }
                    }
                }
            } else if self.after_escape {
                self.after_escape = false;
                if byte == b']' {
                    self.pending = Some(Vec::new());
                    // The sequence began at the ESC, one byte earlier.
                    self.pending_start = start - 1;
                }
            } else if byte == 0x1b {
                self.after_escape = true;
            }
        }
        self.offset += bytes.len();
        events
    }

    fn finish(&mut self, events: &mut Vec<(usize, ShellEvent)>) {
        self.after_escape = false;
        let Some(payload) = self.pending.take() else {
            return;
        };
        if let Some(event) = parse_payload(&payload) {
            // Offsets are relative to the current call; a sequence begun in an earlier call
            // is reported at the start of this one.
            events.push((self.pending_start.saturating_sub(self.offset), event));
        }
    }
}

fn parse_payload(payload: &[u8]) -> Option<ShellEvent> {
    let text = std::str::from_utf8(payload).ok()?;
    let (code, rest) = text.split_once(';').unwrap_or((text, ""));
    match code {
        "133" => {
            let mut parts = rest.split(';');
            match parts.next()? {
                "A" => Some(ShellEvent::PromptStart),
                "D" => Some(ShellEvent::CommandEnd(
                    parts.next().and_then(|code| code.trim().parse().ok()),
                )),
                _ => None,
            }
        }
        "7" => file_uri_path(rest).map(ShellEvent::Cwd),
        _ => None,
    }
}

/// Local path from a `file://host/path` URI, with percent-escapes decoded.
pub fn file_uri_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let path = {
        let slash = rest.find('/')?;
        &rest[slash..]
    };
    // Windows paths arrive as `/C:/dir`; drop the leading slash before the drive letter.
    let path = if path.len() > 2 && path.as_bytes()[2] == b':' {
        &path[1..]
    } else {
        path
    };
    let mut decoded = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let digits = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(value) = u8::from_str_radix(digits, 16) {
                decoded.push(value);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).ok()
}

/// Script for PowerShell: wraps `prompt` so every prompt reports the previous exit code,
/// the prompt position and the working directory without Windows verbatim/provider prefixes.
pub const POWERSHELL_INTEGRATION: &str = concat!(
    r#"function global:prompt { "#,
    r#"$code = if ($?) { 0 } else { 1 }; "#,
    r#"$e = [char]27; $b = [char]7; "#,
    r#"$raw = if ($PWD.Provider.Name -eq 'FileSystem') { $PWD.ProviderPath } else { $PWD.Path }; "#,
    r#"$clean = $raw -replace '^Microsoft\.PowerShell\.Core\\FileSystem::', '' -replace '^\\\\\?\\UNC\\', '\\' -replace '^\\\\\?\\', ''; "#,
    r#"$uri_path = $clean -replace '\\', '/'; "#,
    r#"$uri_prefix = if ($uri_path -notmatch '^/') { '/' } else { '' }; "#,
    r#"$pre = if ($global:__graphite_has_prompted) { "$e]133;D;$code$b" } else { $global:__graphite_has_prompted = $true; '' }; "#,
    r#"[Console]::Write("$pre$e]133;A$b$e]7;file://localhost$uri_prefix$uri_path$b"); "#,
    r#""PS $clean> " }"#,
);

/// Prompt command for bash: reports the previous exit code and the prompt position.
pub const BASH_PROMPT_COMMAND: &str = "printf '\\033]133;D;%s\\007\\033]133;A\\007' \"$?\"";

/// Whether a shell executable is one of the shells that gets integration.
#[derive(Debug, PartialEq, Eq)]
pub enum IntegratedShell {
    PowerShell,
    Bash,
    Other,
}

pub fn integrated_shell(path: &str) -> IntegratedShell {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    match name {
        "pwsh" | "powershell" => IntegratedShell::PowerShell,
        "bash" => IntegratedShell::Bash,
        _ => IntegratedShell::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(scanner: &mut ShellScanner, chunks: &[&[u8]]) -> Vec<ShellEvent> {
        chunks
            .iter()
            .flat_map(|chunk| scanner.feed(chunk))
            .map(|(_, event)| event)
            .collect()
    }

    #[test]
    fn prompt_and_exit_code_sequences_are_detected_with_either_terminator() {
        let mut scanner = ShellScanner::default();
        let found = events(
            &mut scanner,
            &[b"ok\x1b]133;D;2\x07\x1b]133;A\x1b\\prompt> "],
        );
        assert_eq!(
            found,
            vec![ShellEvent::CommandEnd(Some(2)), ShellEvent::PromptStart]
        );
    }

    #[test]
    fn sequences_split_across_reads_are_joined() {
        let mut scanner = ShellScanner::default();
        let found = events(
            &mut scanner,
            &[b"\x1b]13", b"3;D;", b"0\x07", b"\x1b", b"]133;A\x07"],
        );
        assert_eq!(
            found,
            vec![ShellEvent::CommandEnd(Some(0)), ShellEvent::PromptStart]
        );
    }

    #[test]
    fn event_offsets_point_at_the_escape_that_starts_them() {
        let mut scanner = ShellScanner::default();
        let found = scanner.feed(b"abc\x1b]133;A\x07de");
        assert_eq!(found, vec![(3, ShellEvent::PromptStart)]);
    }

    #[test]
    fn unrelated_and_oversized_sequences_are_ignored() {
        let mut scanner = ShellScanner::default();
        let mut big = b"\x1b]0;".to_vec();
        big.extend(std::iter::repeat_n(b'x', MAX_PAYLOAD + 10));
        big.push(0x07);
        big.extend_from_slice(b"\x1b[31mred\x1b[0m\x1b]133;A\x07");
        assert_eq!(events(&mut scanner, &[&big]), vec![ShellEvent::PromptStart]);
    }

    #[test]
    fn cwd_uri_is_decoded_to_a_local_path() {
        assert_eq!(
            file_uri_path("file://localhost/C:/Projetos%20x/src"),
            Some("C:/Projetos x/src".to_owned())
        );
        assert_eq!(
            file_uri_path("file://host/home/me"),
            Some("/home/me".to_owned())
        );
        assert_eq!(file_uri_path("http://example.com"), None);
        let mut scanner = ShellScanner::default();
        assert_eq!(
            events(&mut scanner, &[b"\x1b]7;file://h/tmp/a\x07"]),
            vec![ShellEvent::Cwd("/tmp/a".into())]
        );
    }

    #[test]
    fn shells_are_recognized_by_executable_name() {
        assert_eq!(
            integrated_shell(r"C:\Program Files\PowerShell\7\pwsh.exe"),
            IntegratedShell::PowerShell
        );
        assert_eq!(integrated_shell("/usr/bin/bash"), IntegratedShell::Bash);
        assert_eq!(integrated_shell("cmd.exe"), IntegratedShell::Other);
    }

    #[test]
    fn powershell_integration_strips_provider_and_verbatim_prefixes() {
        assert!(POWERSHELL_INTEGRATION.contains(r"Microsoft\.PowerShell\.Core\\FileSystem::"));
        assert!(POWERSHELL_INTEGRATION.contains(r"^\\\\\?\\UNC\\"));
        assert!(POWERSHELL_INTEGRATION.contains(r"^\\\\\?\\"));
        assert!(POWERSHELL_INTEGRATION.contains("__graphite_has_prompted"));
    }
}
