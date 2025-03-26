pub mod generator;
pub mod utils;

pub fn get_int_from_cfg(platform: &str, key: &str) -> Option<usize> {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), key)
}
