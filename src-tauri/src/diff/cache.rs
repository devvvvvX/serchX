use crate::diff::engine::VersionDiff;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

pub struct DiffCache {
    capacity: usize,
    store: Mutex<CacheInner>,
}

struct CacheInner {
    entries: HashMap<(String, String), VersionDiff>,
    order: VecDeque<(String, String)>,
}

impl Default for DiffCache {
    fn default() -> Self {
        Self::new(200)
    }
}

impl DiffCache {
    pub fn new(capacity: usize) -> Self {
        let cap = if capacity == 0 { 200 } else { capacity };
        Self {
            capacity: cap,
            store: Mutex::new(CacheInner {
                entries: HashMap::with_capacity(cap),
                order: VecDeque::with_capacity(cap),
            }),
        }
    }

    pub fn get(&self, version_a: &str, version_b: &str) -> Option<VersionDiff> {
        let mut inner = self.store.lock().ok()?;
        let key = (version_a.to_string(), version_b.to_string());
        if let Some(diff) = inner.entries.get(&key) {
            let res = diff.clone();
            // Move key to back of LRU queue
            if let Some(pos) = inner.order.iter().position(|k| k == &key) {
                inner.order.remove(pos);
            }
            inner.order.push_back(key);
            Some(res)
        } else {
            None
        }
    }

    pub fn put(&self, diff: VersionDiff) {
        let mut inner = match self.store.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let key = (diff.version_a_id.clone(), diff.version_b_id.clone());

        if inner.entries.contains_key(&key) {
            inner.entries.insert(key.clone(), diff);
            if let Some(pos) = inner.order.iter().position(|k| k == &key) {
                inner.order.remove(pos);
            }
            inner.order.push_back(key);
            return;
        }

        if inner.entries.len() >= self.capacity {
            if let Some(oldest_key) = inner.order.pop_front() {
                inner.entries.remove(&oldest_key);
            }
        }

        inner.entries.insert(key.clone(), diff);
        inner.order.push_back(key);
    }

    pub fn len(&self) -> usize {
        self.store.lock().map(|i| i.entries.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut inner) = self.store.lock() {
            inner.entries.clear();
            inner.order.clear();
        }
    }
}
