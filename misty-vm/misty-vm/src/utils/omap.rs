use std::collections::HashMap;
use std::slice::Iter as SliceIter;

#[derive(Debug)]
pub struct OMap<K, V> {
    keys: Vec<K>,
    map: HashMap<K, V>,
}

impl<K, V> OMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    pub fn new() -> Self {
        OMap {
            keys: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if !self.map.contains_key(&key) {
            self.keys.push(key.clone());
        }
        self.map.insert(key, value)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    pub fn keys(&self) -> SliceIter<'_, K> {
        self.keys.iter()
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            keys: self.keys.iter(),
            map: &self.map,
        }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

pub struct Iter<'a, K, V> {
    keys: SliceIter<'a, K>,
    map: &'a HashMap<K, V>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: Eq + std::hash::Hash,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.keys
            .next()
            .map(|key| (key, self.map.get(key).unwrap()))
    }
}

impl<K, V> std::default::Default for OMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    fn default() -> Self {
        OMap {
            keys: Vec::new(),
            map: HashMap::new(),
        }
    }
}
