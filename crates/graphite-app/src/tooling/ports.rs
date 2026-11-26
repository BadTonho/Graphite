//! Local listener models. Only Graphite-owned workspace processes are eligible
//! for termination; this module never kills an arbitrary system process.

use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalPort {
    pub address: String,
    pub port: u16,
    pub process_id: u32,
    pub state: String,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_windows_netstat(text: &str, owned: &BTreeSet<u32>) -> Vec<LocalPort> {
    let mut listeners = Vec::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 5
            || !fields[0].eq_ignore_ascii_case("TCP")
            || !fields[3].eq_ignore_ascii_case("LISTENING")
        {
            continue;
        }
        let Ok(process_id) = fields[4].parse::<u32>() else {
            continue;
        };
        if !owned.contains(&process_id) {
            continue;
        }
        let Some((address, port)) = fields[1].rsplit_once(':') else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        listeners.push(LocalPort {
            address: address.trim_matches(['[', ']']).to_owned(),
            port,
            process_id,
            state: "LISTENING".into(),
        });
    }
    listeners.sort_by_key(|item| (item.port, item.process_id));
    listeners.dedup();
    listeners
}

#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn parse_linux_ss(text: &str, owned: &BTreeSet<u32>) -> Vec<LocalPort> {
    let mut listeners = Vec::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 5 || fields[0] != "LISTEN" {
            continue;
        }
        let process_id = fields.iter().find_map(|field| {
            let start = field.find("pid=")? + 4;
            field[start..]
                .split(|ch: char| !ch.is_ascii_digit())
                .next()?
                .parse::<u32>()
                .ok()
        });
        let Some(process_id) = process_id.filter(|pid| owned.contains(pid)) else {
            continue;
        };
        let Some((address, port)) = fields[3].rsplit_once(':') else {
            continue;
        };
        let Ok(port) = port.parse::<u16>() else {
            continue;
        };
        listeners.push(LocalPort {
            address: address.trim_matches(['[', ']']).to_owned(),
            port,
            process_id,
            state: "LISTEN".into(),
        });
    }
    listeners.sort_by_key(|item| (item.port, item.process_id));
    listeners.dedup();
    listeners
}

#[derive(Default)]
pub(crate) struct PortManager {
    owned_processes: BTreeSet<u32>,
    pub listeners: Vec<LocalPort>,
}

impl PortManager {
    pub(crate) fn register_process(&mut self, process_id: u32) {
        if process_id > 0 {
            self.owned_processes.insert(process_id);
        }
    }

    pub(crate) fn can_stop(&self, process_id: u32) -> bool {
        self.owned_processes.contains(&process_id)
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn refresh_from_netstat(&mut self, text: &str) {
        self.listeners = parse_windows_netstat(text, &self.owned_processes);
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub(crate) fn refresh_from_ss(&mut self, text: &str) {
        self.listeners = parse_linux_ss(text, &self.owned_processes);
    }

    pub(crate) fn remove_process(&mut self, process_id: u32) {
        self.owned_processes.remove(&process_id);
        self.listeners
            .retain(|listener| listener.process_id != process_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_workspace_process_listeners_and_never_authorizes_other_processes() {
        let mut manager = PortManager::default();
        manager.register_process(1234);
        manager.refresh_from_netstat("  TCP    127.0.0.1:3000  0.0.0.0:0  LISTENING  1234\n  TCP    0.0.0.0:80  0.0.0.0:0  LISTENING  99\n  TCP    127.0.0.1:44  0.0.0.0:0  ESTABLISHED  1234");
        assert_eq!(manager.listeners.len(), 1);
        assert_eq!(manager.listeners[0].port, 3000);
        assert!(manager.can_stop(1234));
        assert!(!manager.can_stop(99));
        manager.remove_process(1234);
        assert!(manager.listeners.is_empty());
    }

    #[test]
    fn refreshes_linux_listeners_through_the_manager() {
        let mut manager = PortManager::default();
        manager.register_process(7);
        manager.refresh_from_ss("LISTEN 0 128 [::1]:5173 [::]:* users:((\"vite\",pid=7,fd=11))\nLISTEN 0 64 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=99,fd=3))");
        assert_eq!(manager.listeners.len(), 1);
        assert_eq!(manager.listeners[0].address, "::1");
        assert!(!manager.can_stop(99));
    }

    #[test]
    fn parses_linux_listener_rows_and_filters_unowned_processes() {
        let owned = BTreeSet::from([7]);
        let listeners = parse_linux_ss("State Recv-Q Send-Q Local Address:Port Peer Address:Port Process\nLISTEN 0 128 127.0.0.1:5173 0.0.0.0:* users:((\"vite\",pid=7,fd=11))\nLISTEN 0 64 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=99,fd=3))", &owned);
        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0].port, 5173);
    }
}
