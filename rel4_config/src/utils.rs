#![allow(unused)]

use std::env::VarError;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use serde_yaml::Value;

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
    file.read_to_string(&mut contents).expect("Unable to read file");

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
    file.read_to_string(&mut contents).expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_u64().map(|n| n as usize)
}

#[derive(serde::Deserialize)]
pub struct MemZone {
    pub start: usize,
    pub end: usize,
}

pub(crate) fn get_zone_from_yaml(file_path: &str, key: &str) -> Option<Vec<MemZone>> {
    let mut file = File::open(file_path).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Unable to read file");

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