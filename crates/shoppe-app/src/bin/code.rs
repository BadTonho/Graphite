use std::{
    env,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let wait = args.iter().any(|arg| arg == "--wait" || arg == "-w");
    let Ok(executable) = env::current_exe() else {
        eprintln!("Could not locate the VsCodeShoppe installation");
        return ExitCode::FAILURE;
    };
    let app = executable.with_file_name(if cfg!(windows) {
        "shoppe-app.exe"
    } else {
        "shoppe-app"
    });
    let mut child = match Command::new(app).args(args).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("Could not start VsCodeShoppe: {error}");
            return ExitCode::FAILURE;
        }
    };
    if wait {
        match child.wait() {
            Ok(status) if status.success() => ExitCode::SUCCESS,
            Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8),
            Err(error) => {
                eprintln!("Could not wait for the editor: {error}");
                ExitCode::FAILURE
            }
        }
    } else {
        ExitCode::SUCCESS
    }
}
