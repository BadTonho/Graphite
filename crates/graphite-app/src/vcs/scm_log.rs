//! A log of the git commands that changed something, shown by **Git: Show Git Output**. Reads
//! (status, log, listings) are not recorded: they run on every refresh and would bury the rest.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many lines are kept.
const CAPACITY: usize = 1000;

static LINES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// Whether `args` is a command that changes the repository or talks to a remote.
fn changes_something(args: &[&str]) -> bool {
    let rest = &args[args.len().min(1)..];
    match args.first().copied().unwrap_or_default() {
        "commit" | "add" | "push" | "pull" | "fetch" | "checkout" | "switch" | "merge"
        | "rebase" | "cherry-pick" | "revert" | "reset" | "restore" | "clean" | "init"
        | "clone" | "apply" => true,
        "branch" | "tag" => !rest.iter().any(|arg| matches!(*arg, "--list" | "-l")),
        "stash" => rest.first().copied() != Some("list"),
        "remote" => matches!(rest.first().copied(), Some("add" | "remove" | "rename" | "set-url")),
        _ => false,
    }
}

fn clock() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let of_day = seconds % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// Records a command and how it ended (`Ok` with its output or `Err` with the reason).
pub(crate) fn record(args: &[&str], outcome: Result<&str, &str>) {
    if !changes_something(args) {
        return;
    }
    let now = clock();
    let mut lines = LINES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut push = |line: String| {
        if lines.len() == CAPACITY {
            lines.pop_front();
        }
        lines.push_back(line);
    };
    push(format!("[{now}] > git {}", args.join(" ")));
    match outcome {
        Ok(output) => {
            for line in output.lines().filter(|line| !line.trim().is_empty()).take(20) {
                push(format!("           {line}"));
            }
        }
        Err(reason) => {
            for line in reason.lines().filter(|line| !line.trim().is_empty()).take(20) {
                push(format!("           error: {line}"));
            }
        }
    }
}

/// The whole log, oldest line first.
pub(crate) fn text() -> String {
    let lines = LINES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    lines.iter().cloned().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_commands_that_change_something_are_recorded() {
        for read in [
            vec!["status"],
            vec!["log", "-1"],
            vec!["rev-parse", "HEAD"],
            vec!["for-each-ref"],
            vec!["stash", "list"],
            vec!["tag", "--list"],
            vec!["branch", "--list"],
            vec!["remote"],
            vec!["check-ref-format", "x"],
            vec![],
        ] {
            assert!(!changes_something(&read), "{read:?}");
        }
        for write in [
            vec!["commit", "-m", "x"],
            vec!["push"],
            vec!["stash", "push"],
            vec!["tag", "v1"],
            vec!["branch", "-D", "x"],
            vec!["remote", "add", "o", "u"],
            vec!["restore", "--", "a"],
        ] {
            assert!(changes_something(&write), "{write:?}");
        }
    }

    #[test]
    fn a_recorded_command_shows_its_outcome() {
        record(&["push", "--set-upstream", "log-test-remote", "HEAD"], Err("rejected\nnon-fast-forward"));
        record(&["commit", "-m", "log test message"], Ok("[main abc123] log test message"));
        record(&["status"], Ok("not recorded"));
        let text = text();
        assert!(text.contains("> git push --set-upstream log-test-remote HEAD"));
        assert!(text.contains("error: rejected"));
        assert!(text.contains("error: non-fast-forward"));
        assert!(text.contains("> git commit -m log test message"));
        assert!(text.contains("[main abc123] log test message"));
        assert!(!text.contains("not recorded"));
        let push = text.find("log-test-remote").unwrap();
        let commit = text.find("log test message").unwrap();
        assert!(push < commit, "oldest first");
    }

    #[test]
    fn the_log_keeps_only_the_latest_lines() {
        for index in 0..CAPACITY + 50 {
            record(&["add", &format!("capacity-test-{index}")], Ok(""));
        }
        let text = text();
        assert!(text.lines().count() <= CAPACITY);
        assert!(!text.contains("capacity-test-0\n") && text.contains(&format!("capacity-test-{}", CAPACITY + 49)));
    }

    #[test]
    fn the_clock_has_the_form_hh_mm_ss() {
        let now = clock();
        assert_eq!(now.len(), 8);
        assert_eq!(now.matches(':').count(), 2);
    }
}
