//! Bounded ownership of material bindings and structural resource variants.
//! Descriptor snapshots keep their own Arc references in command buffers;
//! eviction drops cache ownership without changing resources used by a GPU frame.
use std::collections::HashMap;
use std::hash::Hash;

pub(crate) struct MaterialCache<K, V> {
    entries: HashMap<K, (u64, V)>,
    capacity: usize,
    clock: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl<K: Copy + Eq + Hash, V> MaterialCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            entries: HashMap::new(),
            capacity,
            clock: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.clock += 1;
        if let Some((last_used, value)) = self.entries.get_mut(key) {
            *last_used = self.clock;
            self.hits += 1;
            Some(value)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.len() == self.capacity && !self.entries.contains_key(&key) {
            let oldest = *self
                .entries
                .iter()
                .min_by_key(|(_, (age, _))| *age)
                .unwrap()
                .0;
            self.entries.remove(&oldest);
            self.evictions += 1;
        }
        self.clock += 1;
        self.entries.insert(key, (self.clock, value));
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.clock += 1;
        if let Some((last_used, value)) = self.entries.get_mut(key) {
            *last_used = self.clock;
            self.hits += 1;
            Some(value)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        self.entries.retain(|key, (_, value)| keep(key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn material_cache_bounds_continuous_edits_and_preserves_in_flight_ownership() {
        let mut cache = MaterialCache::new(32);
        let in_flight = Arc::new(0);
        cache.insert(0, in_flight.clone());
        for value in 1..10_000 {
            cache.insert(value, Arc::new(value));
            assert!(cache.len() <= 32);
        }
        assert_eq!(*in_flight, 0);
        assert_eq!(Arc::strong_count(&in_flight), 1);
        assert_eq!(cache.evictions, 10_000 - 32);
        assert!(cache.get(&0).is_none());
        assert_eq!(**cache.get(&9_999).unwrap(), 9_999);
    }

    #[test]
    fn material_cache_keeps_recently_used_entries_and_invalidates_resources() {
        let mut cache = MaterialCache::new(2);
        cache.insert((1, 0), 10);
        cache.insert((2, 0), 20);
        assert_eq!(cache.get(&(1, 0)), Some(&10));
        cache.insert((3, 0), 30);
        assert!(cache.get(&(2, 0)).is_none());
        cache.retain(|(texture, _), _| *texture != 1);
        assert!(cache.get(&(1, 0)).is_none());
        assert_eq!(cache.len(), 1);
    }
}
