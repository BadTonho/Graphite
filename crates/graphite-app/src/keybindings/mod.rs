//! Parsing and resolving user keybindings independently of the event loop.
mod defaults;
mod when;
pub(crate) use when::Expr as Condition;

use graphite_i18n::Translator;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct Binding {
    keys: Vec<String>,
    command: String,
    condition: Option<when::Expr>,
    arguments: serde_json::Value,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    Unmatched,
    Pending,
    Command {
        id: String,
        arguments: serde_json::Value,
    },
    Unbound,
}

#[derive(Debug)]
pub(crate) struct Keymap {
    defaults: Vec<Binding>,
    plugin_bindings: Vec<Binding>,
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
                arguments: serde_json::Value::Null,
                condition: Some(when::parse(condition).expect("valid default context")),
            })
            .collect();
        Self {
            defaults,
            plugin_bindings: Vec::new(),
            bindings: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl Keymap {
    /// Return the shortcut currently configured for a command, preferring a
    /// profile binding over its default binding.
    pub(crate) fn configured_shortcut(&self, command: &str) -> Option<String> {
        let custom = self
            .bindings
            .iter()
            .rev()
            .find(|binding| binding.command == command);
        if let Some(binding) = custom {
            return Some(format_binding_keys(&binding.keys));
        }
        let binding = self
            .defaults
            .iter()
            .chain(&self.plugin_bindings)
            .find(|binding| {
                binding.command == command
                    && !self.bindings.iter().any(|custom| {
                        custom.keys == binding.keys
                            && (custom.command == format!("-{command}")
                                || custom.command != command)
                    })
            })?;
        Some(format_binding_keys(&binding.keys))
    }

    pub(crate) fn set_plugin_bindings(&mut self, specs: &[(String, String, Option<String>)]) {
        self.plugin_bindings = specs
            .iter()
            .filter_map(|(key, command, when)| {
                let keys = key
                    .split_whitespace()
                    .map(normalize_stroke)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                if keys.is_empty() || command.trim().is_empty() {
                    return None;
                }
                let condition = match when {
                    Some(expr) => when::parse(expr).ok(),
                    None => None,
                };
                Some(Binding {
                    keys,
                    command: command.clone(),
                    arguments: serde_json::Value::Null,
                    condition,
                })
            })
            .collect();
    }

    pub(crate) fn reference_bindings(&self) -> Vec<(String, String)> {
        let mut rows = self
            .defaults
            .iter()
            .chain(&self.plugin_bindings)
            .filter(|binding| {
                !self.bindings.iter().any(|custom| {
                    custom.keys == binding.keys
                        && (custom.command == format!("-{}", binding.command)
                            || custom.command != binding.command)
                })
            })
            .map(|binding| (binding.command.clone(), format_binding_keys(&binding.keys)))
            .collect::<Vec<_>>();
        rows.extend(
            self.bindings
                .iter()
                .filter(|binding| !binding.command.starts_with('-'))
                .map(|binding| (binding.command.clone(), format_binding_keys(&binding.keys))),
        );
        rows.sort();
        rows.dedup();
        rows
    }

    /// Replace all user bindings only after the complete file validates.
    pub(crate) fn reload(&mut self, json: &str, translator: &Translator) -> Result<(), String> {
        let value = crate::config::parse_jsonc(json).map_err(|e| e.to_string())?;
        let entries = value
            .as_array()
            .ok_or_else(|| translator.text("config.keybinding.array"))?;
        let mut bindings = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let parse = || -> Result<Binding, String> {
                let object = entry
                    .as_object()
                    .ok_or_else(|| translator.text("config.keybinding.entry_object"))?;
                let key = object
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| translator.text("config.keybinding.missing_key"))?;
                let command = object
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| translator.text("config.keybinding.missing_command"))?;
                if command.trim().is_empty() {
                    return Err(translator.text("config.keybinding.empty_command").into());
                }
                let keys = key
                    .split_whitespace()
                    .map(|stroke| normalize_stroke_with(stroke, translator))
                    .collect::<Result<Vec<_>, _>>()?;
                if keys.is_empty() {
                    return Err(translator.text("config.keybinding.empty_key").into());
                }
                let condition = match object.get("when") {
                    Some(value) => Some(when::parse_with(
                        value
                            .as_str()
                            .ok_or_else(|| translator.text("config.keybinding.when_string"))?,
                        translator,
                    )?),
                    None => None,
                };
                Ok(Binding {
                    keys,
                    command: command.to_owned(),
                    arguments: object.get("args").cloned().unwrap_or_default(),
                    condition,
                })
            };
            bindings.push(parse().map_err(|error| {
                translator.format(
                    "config.keybinding.entry",
                    &[("index", &(index + 1)), ("error", &error)],
                )
            })?);
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
            let all: Vec<_> = self
                .defaults
                .iter()
                .chain(&self.plugin_bindings)
                .chain(&self.bindings)
                .collect();
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
                return Resolution::Command {
                    id: command,
                    arguments: binding.arguments.clone(),
                };
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

fn format_binding_keys(keys: &[String]) -> String {
    keys.iter()
        .map(|stroke| {
            stroke
                .split('+')
                .map(|key| match key {
                    "ctrl" => "Ctrl".to_owned(),
                    "shift" => "Shift".to_owned(),
                    "alt" => "Alt".to_owned(),
                    "meta" => "Meta".to_owned(),
                    other => other
                        .chars()
                        .next()
                        .map(|first| first.to_uppercase().to_string() + &other[first.len_utf8()..])
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn normalize_stroke(stroke: &str) -> Result<String, String> {
    normalize_stroke_with(stroke, &Translator::english())
}

fn normalize_stroke_with(stroke: &str, translator: &Translator) -> Result<String, String> {
    let mut modifiers = [false; 4];
    let mut key = None;
    for part in stroke.split('+') {
        let part = part.to_ascii_lowercase();
        match part.as_str() {
            "ctrl" | "control" => modifiers[0] = true,
            "shift" => modifiers[1] = true,
            "alt" | "option" => modifiers[2] = true,
            "meta" | "cmd" | "win" | "super" => modifiers[3] = true,
            "" => return Err(translator.text("config.keybinding.empty_component").into()),
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
            _ => return Err(translator.text("config.keybinding.multiple_keys").into()),
        }
    }
    let key = key.ok_or_else(|| translator.text("config.keybinding.missing_stroke_key"))?;
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

    fn translator() -> Translator {
        Translator::english()
    }

    #[test]
    fn chords_context_precedence_and_removal() {
        let mut map = Keymap::default();
        map.reload(
            r#"[
            {"key":"ctrl+k ctrl+x", "command":"first"},
            {"key":"ctrl+k ctrl+x", "command":"second", "when":"editorFocus && !readonly"},
            {"key":"alt+up", "command":"-old"}
        ]"#,
            &translator(),
        )
        .unwrap();
        let context = HashMap::from([("editorFocus".into(), "true".into())]);
        assert_eq!(map.resolve("Ctrl+K", &context), Resolution::Pending);
        assert_eq!(
            map.resolve("ctrl+x", &context),
            Resolution::Command {
                id: "second".into(),
                arguments: serde_json::Value::Null
            }
        );
        assert_eq!(map.resolve("alt+arrowup", &context), Resolution::Unbound);
        assert_eq!(map.resolve("ctrl+k", &context), Resolution::Pending);
        assert_eq!(map.resolve("alt+up", &context), Resolution::Unbound);
    }

    #[test]
    fn invalid_reload_keeps_previous_bindings() {
        let mut map = Keymap::default();
        map.reload(r#"[{"key":"ctrl+p","command":"open"}]"#, &translator())
            .unwrap();
        assert!(map
            .reload(
                r#"[{"key":"ctrl+x","command":"cut","when":"("}]"#,
                &translator(),
            )
            .is_err());
        assert_eq!(
            map.resolve("ctrl+p", &HashMap::new()),
            Resolution::Command {
                id: "open".into(),
                arguments: serde_json::Value::Null
            }
        );
    }

    #[test]
    fn defaults_support_context_chords_overrides_and_targeted_removal() {
        let mut map = Keymap::default();
        let editor = HashMap::from([("editorTextFocus".into(), "true".into())]);
        assert_eq!(
            map.resolve("ctrl+s", &editor),
            Resolution::Command {
                id: "workbench.action.files.save".into(),
                arguments: serde_json::Value::Null
            }
        );
        assert_eq!(map.resolve("ctrl+k", &editor), Resolution::Pending);
        assert_eq!(
            map.resolve("ctrl+\\", &editor),
            Resolution::Command {
                id: "workbench.action.splitEditorDown".into(),
                arguments: serde_json::Value::Null
            }
        );
        map.reload(
            r#"[
          {"key":"ctrl+s","command":"-workbench.action.files.save"},
          {"key":"ctrl+s","command":"other", "when":"editorReadonly"},
          {"key":"ctrl+q","command":"workbench.action.files.save"}
        ]"#,
            &translator(),
        )
        .unwrap();
        assert_eq!(map.resolve("ctrl+s", &editor), Resolution::Unbound);
        assert_eq!(
            map.resolve("ctrl+q", &editor),
            Resolution::Command {
                id: "workbench.action.files.save".into(),
                arguments: serde_json::Value::Null
            }
        );
        let readonly = HashMap::from([
            ("editorTextFocus".into(), "true".into()),
            ("editorReadonly".into(), "true".into()),
        ]);
        assert_eq!(
            map.resolve("ctrl+s", &readonly),
            Resolution::Command {
                id: "other".into(),
                arguments: serde_json::Value::Null
            }
        );
    }

    #[test]
    fn removing_an_unrelated_command_does_not_mask_the_default() {
        let mut map = Keymap::default();
        map.reload(
            r#"[{"key":"ctrl+s","command":"-unrelated"}]"#,
            &translator(),
        )
        .unwrap();
        let editor = HashMap::from([("editorTextFocus".into(), "true".into())]);
        assert_eq!(
            map.resolve("ctrl+s", &editor),
            Resolution::Command {
                id: "workbench.action.files.save".into(),
                arguments: serde_json::Value::Null
            }
        );
    }
}

/// The default bindings as the JSON list a keybindings file uses, for showing them read-only.
pub(crate) fn default_keybindings_json() -> String {
    let entries: Vec<serde_json::Value> = defaults::BINDINGS
        .iter()
        .map(|(key, command, when)| {
            let mut entry = serde_json::json!({"key": key, "command": command});
            if *when != "true" {
                entry["when"] = serde_json::json!(when);
            }
            entry
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_default() + "\n"
}

#[cfg(test)]
pub(crate) fn default_command_ids() -> Vec<&'static str> {
    defaults::BINDINGS.iter().map(|(_, id, _)| *id).collect()
}
