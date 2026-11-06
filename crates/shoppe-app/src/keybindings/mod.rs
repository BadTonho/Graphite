//! Parsing and resolving user keybindings independently of the event loop.
mod defaults;
mod when;

use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Binding {
    keys: Vec<String>,
    command: String,
    condition: Option<when::Expr>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    Unmatched,
    Pending,
    Command(String),
    Unbound,
}

pub(crate) struct Keymap {
    defaults: Vec<Binding>,
    bindings: Vec<Binding>,
    pending: Vec<String>,
}

impl Default for Keymap {
    fn default() -> Self {
        let defaults = defaults::BINDINGS
            .iter()
            .map(|(key, command, condition)| Binding {
                keys: key
                    .split_whitespace()
                    .map(|stroke| normalize_stroke(stroke).expect("valid default key"))
                    .collect(),
                command: (*command).into(),
                condition: Some(when::parse(condition).expect("valid default context")),
            })
            .collect();
        Self {
            defaults,
            bindings: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl Keymap {
    /// Replace all user bindings only after the complete file validates.
    pub(crate) fn reload(&mut self, json: &str) -> Result<(), String> {
        let value = crate::config::parse_jsonc(json).map_err(|e| e.to_string())?;
        let entries = value.as_array().ok_or("keybindings must be an array")?;
        let mut bindings = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let parse = || -> Result<Binding, String> {
                let object = entry.as_object().ok_or("entry must be an object")?;
                let key = object
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or("missing key")?;
                let command = object
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("missing command")?;
                if command.trim().is_empty() {
                    return Err("empty command".into());
                }
                let keys = key
                    .split_whitespace()
                    .map(normalize_stroke)
                    .collect::<Result<Vec<_>, _>>()?;
                if keys.is_empty() {
                    return Err("empty key".into());
                }
                let condition = match object.get("when") {
                    Some(value) => {
                        Some(when::parse(value.as_str().ok_or("when must be a string")?)?)
                    }
                    None => None,
                };
                Ok(Binding {
                    keys,
                    command: command.to_owned(),
                    condition,
                })
            };
            bindings.push(parse().map_err(|error| format!("keybinding {}: {error}", index + 1))?);
        }
        self.bindings = bindings;
        self.pending.clear();
        Ok(())
    }

    pub(crate) fn resolve(
        &mut self,
        stroke: &str,
        context: &HashMap<String, String>,
    ) -> Resolution {
        let Ok(stroke) = normalize_stroke(stroke) else {
            self.pending.clear();
            return Resolution::Unmatched;
        };
        self.pending.push(stroke.clone());
        let mut retried = false;
        loop {
            let all: Vec<_> = self.defaults.iter().chain(&self.bindings).collect();
            let active = |binding: &Binding| {
                binding
                    .condition
                    .as_ref()
                    .is_none_or(|condition| condition.evaluate(context))
            };
            let matching = all.iter().enumerate().rev().find_map(|(index, binding)| {
                if binding.command.starts_with('-')
                    || !binding.keys.starts_with(&self.pending)
                    || !active(binding)
                {
                    return None;
                }
                let removed = all[index + 1..].iter().any(|removal| {
                    removal.keys == binding.keys
                        && removal.command.strip_prefix('-') == Some(binding.command.as_str())
                        && active(removal)
                });
                (!removed).then_some(*binding)
            });
            if let Some(binding) = matching {
                if binding.keys.len() > self.pending.len() {
                    return Resolution::Pending;
                }
                let command = binding.command.clone();
                self.pending.clear();
                return Resolution::Command(command);
            }
            if all.iter().any(|binding| {
                binding.command.starts_with('-') && binding.keys == self.pending && active(binding)
            }) {
                self.pending.clear();
                return Resolution::Unbound;
            }
            if self.pending.len() > 1 && !retried {
                self.pending = vec![stroke.clone()];
                retried = true;
            } else {
                self.pending.clear();
                return Resolution::Unmatched;
            }
        }
    }
}

pub(crate) fn normalize_stroke(stroke: &str) -> Result<String, String> {
    let mut modifiers = [false; 4];
    let mut key = None;
    for part in stroke.split('+') {
        let part = part.to_ascii_lowercase();
        match part.as_str() {
            "ctrl" | "control" => modifiers[0] = true,
            "shift" => modifiers[1] = true,
            "alt" | "option" => modifiers[2] = true,
            "meta" | "cmd" | "win" | "super" => modifiers[3] = true,
            "" => return Err("empty key component".into()),
            _ if key.is_none() => {
                key = Some(match part.as_str() {
                    "up" => "arrowup".into(),
                    "down" => "arrowdown".into(),
                    "left" => "arrowleft".into(),
                    "right" => "arrowright".into(),
                    "esc" => "escape".into(),
                    _ => part,
                })
            }
            _ => return Err("more than one key in a stroke".into()),
        }
    }
    let key = key.ok_or("missing key in stroke")?;
    let mut result = String::new();
    for (enabled, name) in modifiers
        .into_iter()
        .zip(["ctrl+", "shift+", "alt+", "meta+"])
    {
        if enabled {
            result.push_str(name);
        }
    }
    result.push_str(&key);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_context_precedence_and_removal() {
        let mut map = Keymap::default();
        map.reload(
            r#"[
            {"key":"ctrl+k ctrl+x", "command":"first"},
            {"key":"ctrl+k ctrl+x", "command":"second", "when":"editorFocus && !readonly"},
            {"key":"alt+up", "command":"-old"}
        ]"#,
        )
        .unwrap();
        let context = HashMap::from([("editorFocus".into(), "true".into())]);
        assert_eq!(map.resolve("Ctrl+K", &context), Resolution::Pending);
        assert_eq!(
            map.resolve("ctrl+x", &context),
            Resolution::Command("second".into())
        );
        assert_eq!(map.resolve("alt+arrowup", &context), Resolution::Unbound);
        assert_eq!(map.resolve("ctrl+k", &context), Resolution::Pending);
        assert_eq!(map.resolve("alt+up", &context), Resolution::Unbound);
    }

    #[test]
    fn invalid_reload_keeps_previous_bindings() {
        let mut map = Keymap::default();
        map.reload(r#"[{"key":"ctrl+p","command":"open"}]"#)
            .unwrap();
        assert!(map
            .reload(r#"[{"key":"ctrl+x","command":"cut","when":"("}]"#)
            .is_err());
        assert_eq!(
            map.resolve("ctrl+p", &HashMap::new()),
            Resolution::Command("open".into())
        );
    }

    #[test]
    fn defaults_support_context_chords_overrides_and_targeted_removal() {
        let mut map = Keymap::default();
        let editor = HashMap::from([("editorTextFocus".into(), "true".into())]);
        assert_eq!(
            map.resolve("ctrl+s", &editor),
            Resolution::Command("workbench.action.files.save".into())
        );
        assert_eq!(map.resolve("ctrl+k", &editor), Resolution::Pending);
        assert_eq!(
            map.resolve("ctrl+\\", &editor),
            Resolution::Command("workbench.action.splitEditorDown".into())
        );
        map.reload(
            r#"[
          {"key":"ctrl+s","command":"-workbench.action.files.save"},
          {"key":"ctrl+s","command":"other", "when":"editorReadonly"},
          {"key":"ctrl+q","command":"workbench.action.files.save"}
        ]"#,
        )
        .unwrap();
        assert_eq!(map.resolve("ctrl+s", &editor), Resolution::Unbound);
        assert_eq!(
            map.resolve("ctrl+q", &editor),
            Resolution::Command("workbench.action.files.save".into())
        );
        let readonly = HashMap::from([
            ("editorTextFocus".into(), "true".into()),
            ("editorReadonly".into(), "true".into()),
        ]);
        assert_eq!(
            map.resolve("ctrl+s", &readonly),
            Resolution::Command("other".into())
        );
    }

    #[test]
    fn removing_an_unrelated_command_does_not_mask_the_default() {
        let mut map = Keymap::default();
        map.reload(r#"[{"key":"ctrl+s","command":"-unrelated"}]"#)
            .unwrap();
        let editor = HashMap::from([("editorTextFocus".into(), "true".into())]);
        assert_eq!(
            map.resolve("ctrl+s", &editor),
            Resolution::Command("workbench.action.files.save".into())
        );
    }
}
