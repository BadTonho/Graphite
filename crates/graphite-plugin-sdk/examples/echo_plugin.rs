use graphite_plugin_sdk::{ChatToolOutcome, Plugin, PluginContext, ServiceResult};
use serde_json::json;

struct EchoPlugin;

impl Plugin for EchoPlugin {
    fn activate(&self, ctx: &PluginContext) -> ServiceResult<()> {
        ctx.register_command("local.rustecho.ping", "Rust Echo Ping", |ctx, args| {
            let files = ctx.find_files(None, None, Some(10))?;
            Ok(json!({
                "pong": true,
                "args": args,
                "fileCount": files.len(),
                "files": files,
            }))
        })?;
        ctx.register_chat_tool(
            "rust_workspace_count",
            "Count files in the workspace via a native Rust plugin.",
            json!({ "type": "object", "properties": {} }),
            |ctx, _input| {
                let files = ctx.find_files(None, None, Some(100))?;
                Ok(ChatToolOutcome {
                    content: format!("workspace_files={}", files.len()),
                    is_error: false,
                })
            },
        )?;
        Ok(())
    }
}

fn main() {
    if let Err(err) = graphite_plugin_sdk::run_plugin(EchoPlugin) {
        eprintln!("echo_plugin fatal error: {err}");
        std::process::exit(1);
    }
}
