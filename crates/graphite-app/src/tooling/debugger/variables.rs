//! The `${...}` variables of launch configurations.

use super::*;

/// What the `${...}` variables of a launch configuration resolve against.
#[derive(Clone, Debug, Default)]
pub struct VariableContext {
    pub workspace: PathBuf,
    /// The file open in the editor, for `${file}` and its relatives.
    pub active_file: Option<PathBuf>,
}

impl VariableContext {
    fn resolve(&self, name: &str) -> Option<String> {
        let text = |path: &Path| path.display().to_string();
        let file = self.active_file.as_deref();
        let file_part = |part: fn(&Path) -> Option<String>| file.and_then(part);
        match name {
            "workspaceFolder" | "cwd" => Some(text(&self.workspace)),
            "workspaceFolderBasename" => self
                .workspace
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            "userHome" => std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).display().to_string()),
            "pathSeparator" | "/" => Some(std::path::MAIN_SEPARATOR.to_string()),
            "file" => file.map(text),
            "fileBasename" => {
                file_part(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()))
            }
            "fileBasenameNoExtension" => {
                file_part(|f| f.file_stem().map(|n| n.to_string_lossy().into_owned()))
            }
            "fileExtname" => file_part(|f| {
                f.extension()
                    .map(|extension| format!(".{}", extension.to_string_lossy()))
            }),
            "fileDirname" => file_part(|f| f.parent().map(|dir| dir.display().to_string())),
            "relativeFile" => file.map(|f| text(f.strip_prefix(&self.workspace).unwrap_or(f))),
            "relativeFileDirname" => file.and_then(|f| {
                f.parent()
                    .map(|dir| text(dir.strip_prefix(&self.workspace).unwrap_or(dir)))
            }),
            _ => name
                .strip_prefix("env:")
                .map(|variable| std::env::var(variable).unwrap_or_default()),
        }
    }
}

/// Replaces `${variable}` references. Unknown variables, and `${file}`-style variables when no
/// file is open, are left in the text so the failure is visible instead of silently empty.
pub fn expand_variables(value: &str, context: &VariableContext) -> String {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            result.push_str(&rest[start..]);
            return result;
        };
        match context.resolve(&after[..end]) {
            Some(replacement) => result.push_str(&replacement),
            None => result.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    result.push_str(rest);
    result
}

pub(crate) fn expand_value(value: &mut Value, context: &VariableContext) {
    match value {
        Value::String(text) => *text = expand_variables(text, context),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| expand_value(item, context)),
        Value::Object(map) => map
            .values_mut()
            .for_each(|item| expand_value(item, context)),
        _ => {}
    }
}
