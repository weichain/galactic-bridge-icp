pub struct HashMapUtils;

impl HashMapUtils {
    pub fn format_keys_as_string<K, V>(keys: &std::collections::HashMap<K, V>) -> String
    where
        K: std::fmt::Display,
    {
        keys.keys()
            .map(|k| k.to_string())
            .collect::<Vec<String>>()
            .join("\n")
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
