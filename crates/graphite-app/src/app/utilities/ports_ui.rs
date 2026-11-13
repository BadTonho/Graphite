use super::*;
use std::process::Command;

impl App {
    pub(crate) fn refresh_local_ports(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        let output = Command::new("netstat.exe")
            .args(["-ano", "-p", "tcp"])
            .output();
        #[cfg(not(windows))]
        let output = Command::new("ss").args(["-ltnp"]).output();
        let output = output.map_err(|error| format!("Could not inspect local ports: {error}"))?;
        if !output.status.success() {
            return Err("Could not inspect local ports".into());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        #[cfg(windows)]
        self.ports.refresh_from_netstat(&text);
        #[cfg(not(windows))]
        self.ports.refresh_from_ss(&text);
        Ok(())
    }

    pub(crate) fn stop_port_process(&mut self, process_id: u32) -> Result<(), String> {
        if !self.ports.can_stop(process_id) {
            return Err("This process was not started by Graphite".into());
        }
        #[cfg(windows)]
        let status = Command::new("taskkill.exe")
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .status();
        #[cfg(not(windows))]
        let status = Command::new("kill")
            .args(["-TERM", &process_id.to_string()])
            .status();
        let status = status.map_err(|error| error.to_string())?;
        if !status.success() {
            return Err("Could not stop the workspace process".into());
        }
        self.ports.remove_process(process_id);
        Ok(())
    }
}
