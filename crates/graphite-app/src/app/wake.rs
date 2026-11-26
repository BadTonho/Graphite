//! Wakes the event loop when a background thread has left work for it.
//!
//! Some background work reports through queues the event loop reads when it runs: finished file
//! writes, debug adapter messages, and messages from web pages. Instead of the loop polling for
//! them many times a second, the thread that fills such a queue calls [`request`]. Requests are
//! coalesced: while one wake-up is on its way, further requests send nothing.

use crate::app::accessibility::UserEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static PROXY: OnceLock<winit::event_loop::EventLoopProxy<UserEvent>> = OnceLock::new();
static PENDING: AtomicBool = AtomicBool::new(false);

/// Lets [`request`] reach the event loop. Called once, when the loop starts.
pub(crate) fn install(proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
    let _ = PROXY.set(proxy);
}

/// Asks the event loop to run soon. Does nothing before [`install`] (in tests, for example) or
/// while an earlier request has not been handled yet.
pub(crate) fn request() {
    let Some(proxy) = PROXY.get() else {
        return;
    };
    if should_send(&PENDING) {
        // The loop is gone only while the app exits; nothing is left to wake then.
        let _ = proxy.send_event(UserEvent::Wake);
    }
}

/// Called when the wake-up arrives, so the next request sends again. The loop polls every
/// queue after any event, so the wake-up itself needs no other handling.
pub(crate) fn handled() {
    PENDING.store(false, Ordering::Release);
}

/// Whether a request should send a wake-up: only the first one until it is handled.
fn should_send(pending: &AtomicBool) -> bool {
    !pending.swap(true, Ordering::AcqRel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_coalesced_until_the_wake_up_is_handled() {
        let pending = AtomicBool::new(false);
        assert!(should_send(&pending));
        assert!(!should_send(&pending));
        assert!(!should_send(&pending));
        pending.store(false, Ordering::Release);
        assert!(should_send(&pending));
    }

    #[test]
    fn a_request_before_the_loop_starts_does_nothing() {
        // No proxy is installed in tests.
        request();
    }
}
