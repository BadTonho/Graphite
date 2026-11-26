use super::*;
use std::sync::atomic::{AtomicU8, Ordering};

/// 0: cancellable, 1: cancelled, 2: commit claimed. A claimed commit must be reconciled.
#[derive(Clone, Default)]
pub(super) struct CommitGate(Arc<AtomicU8>);
impl CommitGate {
    pub fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == 1
    }
    pub fn claim(&self) -> bool {
        self.0
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}
pub(super) struct Written {
    pub document: DocumentId,
    pub result: Result<[u8; 32], String>,
}
#[cfg(test)]
pub(super) struct WriteControl {
    pub ready: mpsc::Sender<()>,
    pub proceed: std::sync::Mutex<mpsc::Receiver<()>>,
    pub after_claim: bool,
}
#[cfg(test)]
impl WriteControl {
    fn wait(&self) {
        self.ready.send(()).unwrap();
        self.proceed
            .lock()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    }
}
pub(super) fn write(
    snapshot: &Snapshot,
    document: DocumentId,
    sender: std::sync::mpsc::Sender<Written>,
    gate: CommitGate,
) {
    let text = snapshot.text.clone();
    let path = snapshot.path.clone();
    let encoding = snapshot.encoding;
    let ending = snapshot.ending;
    #[cfg(test)]
    let control = snapshot.control.clone();
    std::thread::spawn(move || {
        let bytes = crate::file_format::encode(&text, encoding, ending);
        let result = Buffer::write_bytes_with_commit(&path, &bytes, || {
            #[cfg(test)]
            if let Some(control) = &control {
                if !control.after_claim {
                    control.wait();
                }
            }
            let claimed = gate.claim();
            #[cfg(test)]
            if let Some(control) = &control {
                if control.after_claim {
                    control.wait();
                }
            }
            claimed
        })
        .map(|()| crate::file_format::fingerprint(&bytes))
        .map_err(|e| e.to_string());
        let _ = sender.send(Written { document, result });
        crate::app::wake::request();
    });
}
