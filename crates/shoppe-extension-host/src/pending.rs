use graphite_core_api::{protocol::MAX_PENDING, CancellationToken, ServiceError, ServiceResult};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
pub struct RequestState {
    pub cancellation: CancellationToken,
    pub deadline: Option<Instant>,
}
#[derive(Default)]
pub struct PendingRequests {
    next: u64,
    entries: HashMap<u64, RequestState>,
}
impl PendingRequests {
    pub fn insert(&mut self, cancellation: CancellationToken, now: Instant) -> ServiceResult<u64> {
        if cancellation.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        if self.entries.len() >= MAX_PENDING {
            return Err(ServiceError::Busy);
        }
        self.next += 1;
        self.entries.insert(
            self.next,
            RequestState {
                cancellation,
                deadline: Some(now + Duration::from_secs(30)),
            },
        );
        Ok(self.next)
    }
    pub fn get(&self, id: u64) -> Option<&RequestState> {
        self.entries.get(&id)
    }
    pub fn remove(&mut self, id: u64) -> Option<RequestState> {
        self.entries.remove(&id)
    }
    pub fn cancel(&mut self, id: u64) {
        if let Some(state) = self.entries.get(&id) {
            state.cancellation.cancel();
        }
    }
    pub fn suspend(&mut self, id: u64) {
        if let Some(state) = self.entries.get_mut(&id) {
            state.deadline = None;
        }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn expire(&mut self, now: Instant) -> Vec<(u64, ServiceError)> {
        let expired: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(id, s)| {
                if s.cancellation.is_cancelled() {
                    Some((*id, ServiceError::Cancelled))
                } else if s.deadline.is_some_and(|deadline| now >= deadline) {
                    Some((*id, ServiceError::Timeout))
                } else {
                    None
                }
            })
            .collect();
        for (id, _) in &expired {
            if let Some(state) = self.entries.remove(id) {
                state.cancellation.cancel();
            }
        }
        expired
    }
    pub fn drain(&mut self) -> Vec<u64> {
        self.entries
            .drain()
            .map(|(id, s)| {
                s.cancellation.cancel();
                id
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_cancelled_and_expired_requests_finish_once() {
        let now = Instant::now();
        let mut p = PendingRequests::default();
        let first = p.insert(CancellationToken::default(), now).unwrap();
        p.cancel(first);
        assert_eq!(p.expire(now), vec![(first, ServiceError::Cancelled)]);
        assert!(p.expire(now).is_empty());
        for _ in 0..MAX_PENDING {
            p.insert(CancellationToken::default(), now).unwrap();
        }
        assert_eq!(
            p.insert(CancellationToken::default(), now),
            Err(ServiceError::Busy)
        );
        assert_eq!(p.expire(now + Duration::from_secs(30)).len(), MAX_PENDING);
        assert!(p.is_empty());
    }
}
