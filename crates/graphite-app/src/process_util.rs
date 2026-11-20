//! Helpers for starting child processes.

use std::process::Command;

/// `CREATE_NO_WINDOW`: the child gets no console window of its own.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Starts a child without a console window on Windows.
///
/// The editor is a GUI program with no console. Windows gives every console program that such a
/// process starts (`git`, `node`, a language server, `cmd`) a new visible window, unless it is
/// asked not to. Every spawn of a console program must call this. Output is read through pipes,
/// so nothing is lost. On other systems it does nothing.
pub(crate) trait HideConsole {
    fn hide_console(&mut self) -> &mut Self;
}

impl HideConsole for Command {
    fn hide_console(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

/// Lets a GUI-subsystem process print to the terminal that started it.
///
/// Without a console, `eprintln!` goes nowhere. When the editor is started from a terminal, this
/// borrows that terminal's console. Started from the Start menu there is no parent console, the
/// call fails and nothing changes: no window appears. Output redirected to a file or pipe by the
/// parent is kept as it is.
#[cfg(windows)]
pub(crate) fn attach_parent_console() {
    use std::os::windows::io::IntoRawHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_OUTPUT_HANDLE,
    };
    // SAFETY: plain Win32 calls with valid constants; the opened handle is leaked on purpose,
    // because it must stay valid for the life of the process.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return;
        }
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let current = GetStdHandle(which);
            if current.is_null() || current == INVALID_HANDLE_VALUE {
                if let Ok(console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                    SetStdHandle(which, console.into_raw_handle() as _);
                }
            }
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn attach_parent_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_child_still_runs_and_its_output_is_captured() {
        let output = if cfg!(windows) {
            Command::new("cmd")
                .hide_console()
                .args(["/C", "echo hidden-ok"])
                .output()
        } else {
            Command::new("sh")
                .hide_console()
                .args(["-c", "echo hidden-ok"])
                .output()
        }
        .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("hidden-ok"));
    }

    #[test]
    fn attaching_to_the_parent_console_never_panics_without_one() {
        // Tests run with or without a terminal; both must be harmless.
        attach_parent_console();
        attach_parent_console();
    }
}
