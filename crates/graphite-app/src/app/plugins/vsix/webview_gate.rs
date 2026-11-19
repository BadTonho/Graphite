//! Holds messages for an extension webview page until the page has loaded.
//!
//! An extension often posts its first messages right after setting the HTML, before the native
//! page exists or has run its scripts; a page that is not listening yet would lose them. The page
//! reports when it has loaded, and queued messages are delivered then, in order.

use std::collections::{BTreeMap, BTreeSet};

/// Most messages held for one page; the oldest are dropped beyond this.
const MAX_PENDING: usize = 256;

/// The page-side message `window.postMessage` scripts, queued per webview handle.
#[derive(Default)]
pub(crate) struct PageGate {
    loaded: BTreeSet<String>,
    pending: BTreeMap<String, Vec<String>>,
}

impl PageGate {
    /// Returns `script` when the page of `handle` can take it now; otherwise queues it.
    pub(crate) fn post(&mut self, handle: &str, script: String) -> Option<String> {
        if self.loaded.contains(handle) {
            return Some(script);
        }
        let queue = self.pending.entry(handle.to_owned()).or_default();
        if queue.len() == MAX_PENDING {
            queue.remove(0);
        }
        queue.push(script);
        None
    }

    /// The page of `handle` has loaded: returns the queued scripts to run, oldest first.
    pub(crate) fn mark_loaded(&mut self, handle: &str) -> Vec<String> {
        self.loaded.insert(handle.to_owned());
        self.pending.remove(handle).unwrap_or_default()
    }

    /// A new page replaced the one of `handle`; messages wait for it to load.
    pub(crate) fn reset(&mut self, handle: &str) {
        self.loaded.remove(handle);
    }

    /// The webview was disposed.
    pub(crate) fn forget(&mut self, handle: &str) {
        self.loaded.remove(handle);
        self.pending.remove(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_wait_for_the_page_and_are_delivered_in_order() {
        let mut gate = PageGate::default();
        assert_eq!(gate.post("a", "one".into()), None);
        assert_eq!(gate.post("a", "two".into()), None);
        assert_eq!(gate.post("b", "other".into()), None);
        assert_eq!(gate.mark_loaded("a"), ["one", "two"]);
        assert_eq!(gate.post("a", "three".into()), Some("three".into()));
        assert_eq!(gate.mark_loaded("b"), ["other"]);
        assert!(gate.mark_loaded("a").is_empty());
    }

    #[test]
    fn a_replaced_page_holds_messages_again_and_disposal_drops_them() {
        let mut gate = PageGate::default();
        gate.mark_loaded("a");
        gate.reset("a");
        assert_eq!(gate.post("a", "later".into()), None);
        gate.forget("a");
        assert!(gate.mark_loaded("a").is_empty());
        assert_eq!(gate.post("a", "now".into()), Some("now".into()));
    }

    #[test]
    fn the_queue_is_bounded_and_keeps_the_newest_messages() {
        let mut gate = PageGate::default();
        for index in 0..MAX_PENDING + 5 {
            gate.post("a", index.to_string());
        }
        let queued = gate.mark_loaded("a");
        assert_eq!(queued.len(), MAX_PENDING);
        assert_eq!(queued[0], "5");
        assert_eq!(queued.last().unwrap(), &(MAX_PENDING + 4).to_string());
    }
}
