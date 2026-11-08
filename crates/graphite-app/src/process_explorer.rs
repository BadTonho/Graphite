use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProcessMetric {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

pub(crate) fn collect_process_tree(root_pid: u32) -> Vec<ProcessMetric> {
    let Ok(mut system) = System::new() else {
        return Vec::new();
    };
    system.refresh_processes(ProcessesToUpdate::All, true);
    std::thread::sleep(Duration::from_millis(120));
    system.refresh_processes(ProcessesToUpdate::All, true);
    let root = Pid::from_u32(root_pid);
    let mut included = std::collections::HashSet::from([root]);
    loop {
        let before = included.len();
        for (pid, process) in system.processes() {
            if process
                .parent()
                .is_some_and(|parent| included.contains(&parent))
            {
                included.insert(*pid);
            }
        }
        if included.len() == before {
            break;
        }
    }
    let mut metrics = system
        .processes()
        .iter()
        .filter(|(pid, _)| included.contains(pid))
        .map(|(pid, process)| ProcessMetric {
            pid: pid.as_u32(),
            name: process.name().to_string_lossy().into_owned(),
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
        })
        .collect::<Vec<_>>();
    metrics.sort_by_key(|metric| (metric.pid != root_pid, metric.pid));
    metrics
}

pub(crate) fn format_memory(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.0} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub(crate) fn visible_process_range(total: usize, offset: usize, capacity: usize) -> Range<usize> {
    let start = offset.min(total.saturating_sub(capacity));
    start..(start + capacity).min(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_format_is_human_readable() {
        assert_eq!(format_memory(2 * 1024 * 1024), "2 MiB");
        assert_eq!(format_memory(1536 * 1024 * 1024), "1.5 GiB");
    }

    #[test]
    fn current_process_is_visible_in_process_tree() {
        let pid = std::process::id();
        let metrics = collect_process_tree(pid);
        assert!(metrics.iter().any(|metric| metric.pid == pid));
    }

    #[test]
    fn process_panel_window_clamps_scroll_and_exposes_later_rows() {
        assert_eq!(visible_process_range(20, 4, 9), 4..13);
        assert_eq!(visible_process_range(20, 99, 9), 11..20);
        assert_eq!(visible_process_range(3, 2, 9), 0..3);
    }
}
