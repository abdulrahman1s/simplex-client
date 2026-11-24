//! Commands waiting for their answer, by corrId. Entries leave on the answer, on timeout, or when the session ends; the cap bounds the rest.
use std::collections::HashMap;

use crate::smp::error::SmpError;

/// `R` says where the answer goes: a channel or tag for the threaded connection, a tag or nothing in a session.
pub struct Entry<R> {
    pub reply: R,
    pub entity_id: Vec<u8>,
    pub sent_at: u64,
    pub deadline: u64,
    pub is_ping: bool,
}

pub struct Pending<R> {
    map: HashMap<Vec<u8>, Entry<R>>,
    cap: usize,
}

impl<R> Pending<R> {
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            cap,
        }
    }

    pub fn insert(&mut self, corr_id: Vec<u8>, entry: Entry<R>) -> Result<(), SmpError> {
        if self.map.len() >= self.cap {
            return Err(SmpError::TooManyPending);
        }
        if self.map.contains_key(&corr_id) {
            return Err(SmpError::Unexpected("duplicate corrId"));
        }
        self.map.insert(corr_id, entry);
        Ok(())
    }

    pub fn take(&mut self, corr_id: &[u8]) -> Option<Entry<R>> {
        self.map.remove(corr_id)
    }

    /// Removes and returns every entry whose deadline has passed, oldest deadline first.
    pub fn expire(&mut self, now: u64) -> Vec<Entry<R>> {
        let due: Vec<Vec<u8>> = self
            .map
            .iter()
            .filter(|(_, e)| e.deadline <= now)
            .map(|(k, _)| k.clone())
            .collect();
        let mut out: Vec<Entry<R>> = due.iter().filter_map(|k| self.map.remove(k)).collect();
        out.sort_by_key(|e| e.deadline);
        out
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        self.map.values().map(|e| e.deadline).min()
    }

    #[must_use]
    pub fn has_ping(&self) -> bool {
        self.map.values().any(|e| e.is_ping)
    }

    /// Removes every waiter, as when the session ends; the caller answers them outside the lock.
    pub fn take_all(&mut self) -> Vec<Entry<R>> {
        self.map.drain().map(|(_, e)| e).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(deadline: u64) -> Entry<()> {
        Entry {
            reply: (),
            entity_id: vec![],
            sent_at: 0,
            deadline,
            is_ping: false,
        }
    }

    #[test]
    fn pending_entries_leave_on_answer_or_timeout_and_are_capped() {
        let mut p = Pending::new(2);
        p.insert(vec![1], entry(10)).unwrap();
        p.insert(vec![2], entry(20)).unwrap();
        assert_eq!(
            p.insert(vec![3], entry(30)).err(),
            Some(SmpError::TooManyPending)
        );
        assert_eq!(
            p.insert(vec![1], entry(30)).err(),
            Some(SmpError::TooManyPending)
        );
        assert_eq!(p.next_deadline(), Some(10));
        assert!(p.take(&[1]).is_some() && p.take(&[1]).is_none());
        assert_eq!(p.expire(19).len(), 0);
        assert_eq!(p.expire(20).len(), 1);
        assert_eq!(p.next_deadline(), None);
    }

    #[test]
    fn a_corr_id_cannot_be_registered_twice() {
        let mut p = Pending::new(8);
        p.insert(vec![1], entry(10)).unwrap();
        assert_eq!(
            p.insert(vec![1], entry(10)).err(),
            Some(SmpError::Unexpected("duplicate corrId"))
        );
    }
}
