use std::{collections::HashMap, fmt::Display, hash::Hash};

pub struct HashMapUtils;

impl HashMapUtils {
    pub fn format_keys_as_string<K, V>(keys: &HashMap<K, V>) -> String
    where
        K: Display,
    {
        keys.keys()
            .map(|k| k.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn filter<K, V, F>(map: &HashMap<K, V>, filter: F) -> HashMap<K, V>
    where
        K: Clone + Eq + Hash,
        V: Clone,
        F: Fn(&V) -> bool,
    {
        map.iter()
            .filter_map(|(key, value)| {
                if filter(value) {
                    Some((key.clone(), value.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub struct VecUtils;

impl VecUtils {
    pub fn format_keys_as_string<K, V>(keys: &Vec<(K, V)>) -> String
    where
        K: std::fmt::Display,
    {
        keys.iter()
            .map(|(k, _)| k.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }
}
