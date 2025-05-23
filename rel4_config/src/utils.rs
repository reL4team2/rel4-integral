#![allow(unused)]

use serde_yaml::Value;
use std::collections::BTreeMap;
use std::env::VarError;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub fn vec_rustflags() -> Result<Vec<String>, anyhow::Error> {
    match std::env::var("RUSTFLAGS") {
        Ok(s) => Ok(vec![s]),
        Err(e) => match e {
            VarError::NotPresent => Ok(vec![]),
            _ => Err(e.into()),
        },
    }
}

pub(crate) fn get_value_from_yaml(file_path: &str, key: &str) -> Option<String> {
    let mut file = File::open(file_path).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_str().map(|s| s.to_string())
}

pub(crate) fn get_int_from_yaml(file_path: &str, key: &str) -> Option<usize> {
    let mut file = File::open(file_path).expect(file_path);
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_u64().map(|n| n as usize)
}

pub(crate) fn get_all_defs(file_path: &str) -> BTreeMap<String, Option<String>> {
    let mut map = BTreeMap::new();
    let mut file = File::open(file_path).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    if let Some(definitions) = yaml.get("definitions").and_then(|v| v.as_mapping()) {
        for (key, value) in definitions {
            if let Some(key_str) = key.as_str() {
                let entry = match value.as_bool() {
                    Some(true) => Some("".to_string()),
                    Some(false) => None,
                    None => value.as_str().map(|s| s.to_string()),
                };
                map.insert(key_str.to_string(), entry);
            }
        }
    }
    return map;
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct MemZone {
    pub start: usize,
    pub end: usize,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct DeviceRegion {
    pub paddr: usize,
    pub pptr_offset: usize,
    pub arm_execute_never: usize,
    pub user_available: usize,
    pub desc: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Irq {
    pub label: String,
    pub number: usize,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Timer {
    pub label: String,
    pub value: usize,
}

pub(crate) fn get_array_from_yaml<T: serde::de::DeserializeOwned>(
    file_path: &str,
    key: &str,
) -> Option<Vec<T>> {
    let mut file = File::open(file_path).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_sequence().map(|seq| {
        seq.iter()
            .filter_map(|v| serde_yaml::from_value(v.clone()).ok())
            .collect()
    })
}

pub(crate) fn get_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
