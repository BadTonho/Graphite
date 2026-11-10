use super::*;
use crate::app::command_palette::CommandPalette;

pub(super) struct Prompt {
    pub(super) id: u64,
    kind: Kind,
}

enum Kind {
    Input,
    Pick { instance: u64, many: bool },
    Message,
}

impl App {
    pub(super) fn start_vsix_prompt(
        &mut self,
        runtime: &mut Runtime,
        id: u64,
        actor: &str,
        method: &str,
        args: &[Value],
    ) -> Result<bool> {
        let (kind, title, query, password) = match (actor, method) {
            ("MainThreadQuickOpen", "$input") => {
                let options = args.first().unwrap_or(&Value::Null);
                (
                    Kind::Input,
                    options["prompt"]
                        .as_str()
                        .or_else(|| options["placeHolder"].as_str())
                        .unwrap_or("Extension input"),
                    options["value"].as_str().unwrap_or(""),
                    options["password"].as_bool().unwrap_or(false),
                )
            }
            ("MainThreadQuickOpen", "$show") => {
                let instance = args
                    .first()
                    .and_then(Value::as_u64)
                    .ok_or("Invalid quick pick instance")?;
                let options = args.get(1).unwrap_or(&Value::Null);
                (
                    Kind::Pick {
                        instance,
                        many: options["canPickMany"].as_bool().unwrap_or(false),
                    },
                    options["placeHolder"]
                        .as_str()
                        .unwrap_or("Extension quick pick"),
                    "",
                    false,
                )
            }
            ("MainThreadMessageService", "$showMessage")
                if args
                    .get(3)
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty()) =>
            {
                (
                    Kind::Message,
                    args.get(1)
                        .and_then(Value::as_str)
                        .unwrap_or("Extension message"),
                    "",
                    false,
                )
            }
            _ => return Ok(false),
        };
        if runtime.prompt.is_some() {
            return Err("Another extension prompt is open".into());
        }
        runtime.prompt = Some(Prompt { id, kind });
        self.open_command_palette();
        self.command_palette = Some(CommandPalette {
            query: query.into(),
            selected: 0,
            items: if matches!(runtime.prompt.as_ref().unwrap().kind, Kind::Message) {
                choices(&args[3], "title")
            } else {
                Vec::new()
            },
            extension_prompt: Some((title.into(), password)),
        });
        Ok(true)
    }

    pub(super) fn set_vsix_prompt_items(
        &mut self,
        runtime: &Runtime,
        instance: u64,
        items: &Value,
    ) {
        if runtime.prompt.as_ref().is_some_and(|prompt| {
            matches!(prompt.kind, Kind::Pick { instance: current, .. } if current == instance)
        }) {
            if let Some(palette) = &mut self.command_palette {
                palette.items = choices(items, "label");
                palette.selected = 0;
            }
        }
    }

    pub(crate) fn cancel_vsix_prompt(&mut self) {
        self.answer_vsix_prompt(None);
    }

    pub(crate) fn answer_vsix_prompt(&mut self, value: Option<(String, Option<String>)>) {
        let Some(runtime) = &mut self.plugins.vsix else {
            return;
        };
        let Some(prompt) = runtime.prompt.take() else {
            return;
        };
        let mut reply = json!({"type":"reply", "id":prompt.id});
        if let Some((query, selected)) = value {
            let handle = selected.and_then(|id| id.parse::<u64>().ok());
            let result = match prompt.kind {
                Kind::Input => Some(json!(query)),
                Kind::Pick { many: true, .. } => handle.map(|id| json!([id])),
                Kind::Pick { .. } | Kind::Message => handle.map(|id| json!(id)),
            };
            if let Some(result) = result {
                reply["result"] = result;
            }
        }
        if let Some((_, _, deadline)) = &mut runtime.command {
            *deadline = Instant::now() + Duration::from_secs(30);
        }
        if let Err(error) = runtime.session.send(reply) {
            self.status = error;
        }
    }
}

fn choices(items: &Value, label: &str) -> Vec<PaletteItem> {
    items
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(PaletteItem {
                id: item["handle"].as_u64()?.to_string(),
                title: item[label].as_str()?.into(),
                source: "VSIX".into(),
                enabled: true,
            })
        })
        .collect()
}
