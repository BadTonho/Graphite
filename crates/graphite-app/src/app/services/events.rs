use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{sync_channel, SyncSender},
    Arc,
};
struct Subscriber {
    sender: SyncSender<DocumentEvent>,
    cancellation: CancellationToken,
    overflow: Arc<AtomicBool>,
}
#[derive(Default)]
pub(crate) struct DocumentEventHub {
    known: HashMap<DocumentId, DocumentSnapshot>,
    subscribers: HashMap<u64, Subscriber>,
    next: u64,
}
impl DocumentEventHub {
    pub fn reap(&mut self) {
        self.subscribers
            .retain(|_, s| !s.cancellation.is_cancelled());
    }
    pub fn needs_record(&self, id: DocumentId, document: &Document) -> bool {
        self.known.get(&id).is_none_or(|old| {
            old.version != Some(document.buffer.revision())
                || old.dirty != document.is_dirty()
                || old.read_only != document.buffer.is_read_only()
        })
    }
    pub fn record(&mut self, document: DocumentSnapshot) {
        let Some(id) = document.id else {
            return;
        };
        let event = match self.known.get(&id) {
            None => DocumentEvent::Opened(document.clone()),
            Some(old) if old != &document => DocumentEvent::Changed(document.clone()),
            _ => return,
        };
        self.known.insert(id, document);
        self.publish(event);
    }
    fn publish(&mut self, event: DocumentEvent) {
        self.subscribers.retain(|_, subscriber| {
            if subscriber.cancellation.is_cancelled() {
                return false;
            }
            if subscriber.sender.try_send(event.clone()).is_err() {
                subscriber.overflow.store(true, Ordering::Release);
                return false;
            }
            true
        });
    }
    pub fn reconcile(
        &mut self,
        live: std::collections::HashSet<DocumentId>,
        documents: Vec<DocumentSnapshot>,
    ) {
        for document in documents {
            self.record(document);
        }
        let closed: Vec<_> = self
            .known
            .keys()
            .filter(|id| !live.contains(id))
            .copied()
            .collect();
        for id in closed {
            if let Some(doc) = self.known.remove(&id) {
                self.publish(DocumentEvent::Closed(doc));
            }
        }
        self.reap();
    }
    pub fn subscribe(&mut self) -> ServiceResult<DocumentSubscription> {
        self.reap();
        if self.subscribers.len() >= 1024
            || self.known.len() > graphite_core_api::protocol::MAX_PENDING
        {
            return Err(ServiceError::Busy);
        }
        let (sender, receiver) = sync_channel(graphite_core_api::protocol::MAX_PENDING);
        for document in self.known.values() {
            sender
                .try_send(DocumentEvent::Opened(document.clone()))
                .map_err(|_| ServiceError::Busy)?;
        }
        self.next += 1;
        let cancellation = CancellationToken::default();
        let overflow = Arc::new(AtomicBool::new(false));
        self.subscribers.insert(
            self.next,
            Subscriber {
                sender,
                cancellation: cancellation.clone(),
                overflow: overflow.clone(),
            },
        );
        Ok(DocumentSubscription {
            id: self.next,
            receiver,
            cancellation,
            overflow,
        })
    }
}
impl App {
    pub(crate) fn reconcile_document_events(&mut self) {
        let live = self.documents.ids().collect();
        let updates = self
            .documents
            .iter()
            .filter(|document| self.document_events.needs_record(document.id, document))
            .filter_map(|document| snapshot_document(document.id, &document).ok())
            .collect();
        self.document_events.reconcile(live, updates);
    }
}
