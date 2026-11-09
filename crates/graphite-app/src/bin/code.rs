use graphite_i18n::{Language, Translator};
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn translator() -> Translator {
    let language = env::var("GRAPHITE_LANG")
        .ok()
        .and_then(|tag| Language::parse(&tag))
        .unwrap_or_else(Language::detect);
    Translator::new(language)
}

fn app_path(executable: &Path) -> PathBuf {
    executable.with_file_name(if cfg!(windows) {
        "graphite.exe"
    } else {
        "graphite"
    })
}

fn main() -> ExitCode {
    let translator = translator();
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let wait = args.iter().any(|arg| arg == "--wait" || arg == "-w");
    let Ok(executable) = env::current_exe() else {
        eprintln!("{}", translator.text("cli.locate_failed"));
        return ExitCode::FAILURE;
    };
    let app = app_path(&executable);
    let mut child = match Command::new(app).args(args).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!(
                "{}",
                translator.format("cli.start_failed", &[("error", &error)])
            );
            return ExitCode::FAILURE;
        }
    };
    if wait {
        match child.wait() {
            Ok(status) if status.success() => ExitCode::SUCCESS,
            Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8),
            Err(error) => {
                eprintln!(
                    "{}",
                    translator.format("cli.wait_failed", &[("error", &error)])
                );
                ExitCode::FAILURE
            }
        }
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_finds_graphite_beside_itself() {
        let launcher = Path::new("install").join(if cfg!(windows) { "code.exe" } else { "code" });
        assert_eq!(
            app_path(&launcher),
            Path::new("install").join(if cfg!(windows) {
                "graphite.exe"
            } else {
                "graphite"
            })
        );
    }
}
