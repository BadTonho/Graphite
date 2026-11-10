use crate::{
    manifest::{Command, Manifest},
    Result,
};
use std::{fs, path::Path, process::Command as Process};
pub const TYPES: &str = include_str!("../runtime/graphite.d.ts");
pub fn create(destination: &Path) -> Result<()> {
    if destination.exists() {
        return Err("Destination already exists".into());
    }
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Invalid project directory")?
        .to_ascii_lowercase();
    let id = format!("local.{name}");
    let m = Manifest {
        id: id.clone(),
        version: "0.1.0".into(),
        api_version: 1,
        main: "dist/index.js".into(),
        description: "My Graphite plugin".into(),
        commands: vec![Command {
            id: format!("{id}.hello"),
            title: "Hello from my plugin".into(),
        }],
        activity_bar: Vec::new(),
    };
    m.validate()?;
    fs::create_dir_all(destination.join("src")).map_err(|e| e.to_string())?;
    let result = (|| {
        let write = |name: &str, bytes: &[u8]| {
            fs::write(destination.join(name), bytes).map_err(|e| e.to_string())
        };
        write(
            "graphite-plugin.json",
            &serde_json::to_vec_pretty(&m).unwrap(),
        )?;
        write("graphite.d.ts", TYPES.as_bytes())?;
        write("package.json", br#"{"private":true,"type":"module","scripts":{"build":"tsc"},"devDependencies":{"typescript":"^5.9.0"}}"#)?;
        write("tsconfig.json", br#"{"compilerOptions":{"target":"ES2022","module":"NodeNext","moduleResolution":"NodeNext","outDir":"dist","strict":true},"include":["src"]}"#)?;
        write("src/index.ts", format!("import type {{ Context, Graphite }} from '../graphite.js';\nexport async function activate(context: Context, graphite: Graphite) {{\n  context.subscriptions.push(await graphite.commands.registerCommand('{id}.hello', async () => {{\n    const documents = await graphite.documents.list();\n    await graphite.window.showMessage(`Hello! ${{documents.length}} open documents.`);\n  }}));\n}}\nexport function deactivate() {{}}\n").as_bytes())?;
        write("README.md", b"# Graphite plugin\n\nRun `npm install` and `npm run build`, then `graphite plugin pack .`.\nInstall the resulting package from the Plugins panel and enable it after reviewing its code.\nThe package includes compiled JavaScript; installation never runs npm or build scripts.\nUse `graphite.plugins.nodePath` in editor settings to select Node.js 24+.\nCommands must be declared in graphite-plugin.json. Positions use UTF-16 and edits carry document versions.\nPublish the package in a GitHub release and add the graphite-plugin topic to the repository.\n")?;
        write(
            ".gitignore",
            b"node_modules/\ndist/\n.env\n.env.*\n*.graphite-plugin\n",
        )
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}
pub fn check_node(executable: &Path) -> Result<()> {
    let mut command = Process::new(executable);
    command.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot start Node.js: {e}"))?;
    let mut child = child;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Node.js version check timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let version = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_start_matches('v')
        .to_owned();
    if !output.status.success() {
        return Err("Node.js version check failed".into());
    }
    validate_version(&version)
}
fn validate_version(version: &str) -> Result<()> {
    if semver::Version::parse(version)
        .map_err(|e| e.to_string())?
        .major
        < 24
    {
        return Err("Node.js 24 or newer is required".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn runtime_version_requires_node_24() {
        assert!(super::validate_version("22.0.0").is_err());
        assert!(super::validate_version("24.0.0").is_ok());
        assert!(super::validate_version("26.1.0").is_ok());
        assert!(super::validate_version("not-node").is_err());
    }
}
