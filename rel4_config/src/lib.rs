pub mod generator;
pub mod message_label_gen;
pub(crate) mod template;
pub mod utils;

pub fn get_int_from_cfg(platform: &str, hypervisor: bool, key: &str) -> Option<usize> {
    let yaml_cfg = get_platform_yaml_path(platform, hypervisor);
    crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), key)
}

/// Read a definition (from the `definitions` file of a platform) as a string.
/// Boolean `true` yields `Some("")`, boolean `false` yields `None`,
/// string values yield `Some(value)`.
pub fn get_def_from_cfg(platform: &str, key: &str) -> Option<String> {
    let yaml_cfg = resolve_definitions_yaml_path(platform);
    crate::utils::get_all_defs(yaml_cfg.to_str().unwrap())
        .get(key)
        .cloned()
        .flatten()
}

/// Read a boolean definition from a platform (defaults to `false`).
pub fn get_bool_from_cfg(platform: &str, key: &str) -> bool {
    get_def_from_cfg(platform, key).is_some()
}

/// Absolute path of a source platform YAML (cpu/timer/device/memory).
/// In hypervisor mode the `{platform}_hyp.yml` variant is used if it exists;
/// otherwise the plain `{platform}.yml` is used.
pub fn get_platform_yaml_path(platform: &str, hypervisor: bool) -> std::path::PathBuf {
    let base = crate::utils::get_root().join("cfg/platform");
    if hypervisor {
        let hyp = base.join(format!("{}_hyp.yml", platform));
        if hyp.exists() {
            return hyp;
        }
    }
    base.join(format!("{}.yml", platform))
}

/// Absolute path of the source definitions YAML (build configuration).
pub fn get_definitions_yaml_path(platform: &str) -> std::path::PathBuf {
    crate::utils::get_root().join(format!("cfg/definitions/{}.yml", platform))
}

/// Resolve the definitions YAML path to use for code generation: a generated
/// copy (via `GENERATED_DEFINITIONS_YAML`) if present, otherwise the source.
pub fn resolve_definitions_yaml_path(platform: &str) -> std::path::PathBuf {
    if let Ok(path) = std::env::var("GENERATED_DEFINITIONS_YAML") {
        return std::path::PathBuf::from(path);
    }
    get_definitions_yaml_path(platform)
}

/// Generate a YAML file by applying overrides to `src_path` and writing the
/// result to `out_path` (typically under `target/`). The source is untouched.
/// Each override is `(key, value)` where `value` is the literal YAML scalar
/// text to write (e.g. `"true"`, `"false"`, `"\"1\""`).
pub fn generate_yaml(
    src_path: &std::path::Path,
    overrides: &[(String, String)],
    out_path: &std::path::Path,
) -> Result<(), anyhow::Error> {
    let contents = std::fs::read_to_string(src_path)?;
    let out = apply_yaml_overrides(&contents, overrides);

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, out)?;
    Ok(())
}

/// Build the list of definitions overrides from build feature flags. This is
/// shared between the xtask and the build scripts so the generated definitions
/// are always consistent.
pub fn build_definitions_overrides(
    hypervisor: bool,
    pa_40bit: bool,
    mcs: bool,
    smc: bool,
    arm_pcnt: bool,
    arm_ptmr: bool,
    fastpath: bool,
    smp: bool,
    num_nodes: usize,
) -> Vec<(String, String)> {
    let b = |v: bool| v.to_string();
    let s = |v: usize| format!("\"{}\"", v);
    vec![
        ("ARM_HYPERVISOR_SUPPORT".to_string(), b(hypervisor)),
        ("ARM_PA_SIZE_BITS_40".to_string(), b(pa_40bit)),
        ("ARM_PA_SIZE_BITS_44".to_string(), b(!pa_40bit)),
        // 3-level stage-2 only when hypervisor + 40-bit PA.
        ("AARCH64_VSPACE_S2_START_L1".to_string(), b(hypervisor && pa_40bit)),
        ("KERNEL_MCS".to_string(), b(mcs)),
        ("ALLOW_SMC_CALLS".to_string(), b(smc)),
        ("EXPORT_PCNT_USER".to_string(), b(arm_pcnt)),
        ("EXPORT_PTMR_USER".to_string(), b(arm_ptmr)),
        ("FASTPATH".to_string(), b(fastpath)),
        ("ENABLE_SMP_SUPPORT".to_string(), b(smp)),
        ("MAX_NUM_NODES".to_string(), s(num_nodes)),
    ]
}

/// Generate the definitions YAML into `out_dir/definitions.yml` from feature
/// flags, then set `GENERATED_DEFINITIONS_YAML` so `config_gen` reads the
/// generated copy. Returns the generated path.
pub fn generate_definitions_yaml(
    platform: &str,
    overrides: &[(String, String)],
    out_dir: &std::path::Path,
) -> Result<std::path::PathBuf, anyhow::Error> {
    let out_path = out_dir.join("definitions.yml");
    generate_yaml(&get_definitions_yaml_path(platform), overrides, &out_path)?;
    std::env::set_var("GENERATED_DEFINITIONS_YAML", &out_path);
    Ok(out_path)
}

/// Parse a C kernel `gen_config.yaml` (a flat `KEY: value` mapping produced by
/// the seL4 CMake configuration system) into override pairs so the Rust kernel
/// reuses the exact same configuration values as the C components.
///
/// The C file is parsed line-by-line rather than with a YAML parser: it is a
/// flat `KEY: value` file and can contain duplicate keys (platform entries are
/// emitted twice), which a strict YAML parser rejects. On duplicates the last
/// occurrence wins.
///
/// Values are rendered as the literal YAML scalar text expected by
/// `apply_yaml_overrides`: booleans stay unquoted, numbers and strings are
/// quoted. Keys not present in the Rust definitions YAML are simply ignored by
/// the override pass.
pub fn load_c_gen_config(path: &std::path::Path) -> Result<Vec<(String, String)>, anyhow::Error> {
    // Installed configs are JSON (`gen_config.json`); build-tree configs are
    // flat YAML (`gen_config.yaml`). Normalise both to the same override form.
    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        return load_c_gen_config_json(path);
    }
    let contents = std::fs::read_to_string(path)?;
    let mut overrides: Vec<(String, String)> = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let raw_value = raw_value.trim();
        // Strip any trailing inline comment (" #...").
        let value_text = match raw_value.find(" #") {
            Some(i) => raw_value[..i].trim(),
            None => raw_value,
        };
        if value_text.is_empty() {
            continue;
        }
        let normalized = if value_text == "true" || value_text == "false" {
            value_text.to_string()
        } else {
            // Numbers and strings stay quoted (the C config already quotes
            // them); strip/re-add quotes to normalise any bare scalars.
            let unquoted = value_text.trim_matches('"');
            format!("\"{}\"", unquoted)
        };
        // Last occurrence of a duplicate key wins.
        if let Some(existing) = overrides.iter_mut().find(|(k, _)| k == key) {
            existing.1 = normalized;
        } else {
            overrides.push((key.to_string(), normalized));
        }
    }
    Ok(overrides)
}

/// Parse an installed `gen_config.json` (a flat object of bool / quoted-scalar
/// values) into the same `(key, value)` override form as the YAML parser above.
fn load_c_gen_config_json(
    path: &std::path::Path,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    let contents = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path.display(), e))?;
    let serde_json::Value::Object(map) = value else {
        anyhow::bail!("{} is not a JSON object", path.display());
    };
    let mut overrides: Vec<(String, String)> = Vec::new();
    for (key, value) in map {
        let normalized = match value {
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => format!("\"{}\"", n),
            serde_json::Value::String(s) => format!("\"{}\"", s),
            _ => continue,
        };
        overrides.push((key, normalized));
    }
    Ok(overrides)
}

/// Resolve the directory containing the seL4 libsel4 interface XML files, in
/// priority order:
/// 1. `SEL4_INSTALL_DIR` env var — `ninja install` layout, which flattens the
///    interfaces into `<prefix>/libsel4/include` (this is a mode: when set it
///    wins over everything below);
/// 2. `LIBSEL4_DIR` env var — source-tree layout (`kernel/libsel4`);
/// 3. otherwise the standard layout: `<seL4 root>/kernel/libsel4`, where
///    `<seL4 root>` is the parent of `rel4_kernel` (reL4 is a sibling of `kernel/`).
pub fn resolve_libsel4_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("SEL4_INSTALL_DIR") {
        return std::path::Path::new(&dir).join("libsel4/include");
    }
    if let Ok(dir) = std::env::var("LIBSEL4_DIR") {
        return std::path::PathBuf::from(dir);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kernel/libsel4")
}

/// Resolve the C kernel `gen_config` path, in priority order:
/// 1. `SEL4_INSTALL_DIR` — the installed kernel config (`*.json`);
/// 2. `SEL4_KERNEL_GEN_CONFIG` env var (explicit override, `.yaml` or `.json`);
/// 3. derived from the libsel4 dir (the seL4 project's build dir):
///    `<libsel4 dir>/../../build/kernel/gen_config/kernel/gen_config.yaml`;
/// 4. otherwise `None` (fall back to the Rust definitions defaults).
pub fn resolve_gen_config_path() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("SEL4_INSTALL_DIR") {
        let prefix = std::path::Path::new(&dir);
        for candidate in [
            prefix.join("libsel4/include/kernel/gen_config.json"),
            prefix.join("libsel4/include/gen_config.json"),
            prefix.join("kernel/gen_config.json"),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    if let Ok(path) = std::env::var("SEL4_KERNEL_GEN_CONFIG") {
        return Some(std::path::PathBuf::from(path));
    }
    let candidate = resolve_libsel4_dir().join("../../build/kernel/gen_config/kernel/gen_config.yaml");
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

/// Print a build warning for every key present in both `feature_overrides`
/// (CLI/feature flags, which take precedence) and `external` (an imported C
/// `gen_config.yaml`) whose values differ. This surfaces configuration drift
/// between the CLI flags and the imported config file instead of silently
/// letting the CLI value win.
pub fn warn_on_config_conflicts(
    feature_overrides: &[(String, String)],
    external: &[(String, String)],
) {
    for (key, feat_val) in feature_overrides {
        if let Some((_, ext_val)) = external.iter().find(|(k, _)| k == key) {
            if feat_val != ext_val {
                println!(
                    "cargo:warning=config conflict on '{key}': CLI/feature value '{feat_val}' (used) != gen_config.yaml value '{ext_val}' (ignored)"
                );
            }
        }
    }
}

/// Apply text overrides to YAML contents, preserving comments and formatting.
///
/// Each override is one of:
/// - `(key, value)`: replace the `key: ...` scalar value with `value`.
/// - `("!comment", text)`: comment out any line containing `text`.
/// - `("!uncomment", text)`: un-comment any line containing `text`.
/// - `("!replace", "old=>new")`: replace `old` with `new` in any line containing `old`.
fn apply_yaml_overrides(contents: &str, overrides: &[(String, String)]) -> String {
    let mut out = String::with_capacity(contents.len());

    for line in contents.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        let mut handled = false;

        for (key, value) in overrides {
            if key.as_str() == "!comment" || key.as_str() == "!uncomment" {
                if line.contains(value.as_str()) {
                    if key.as_str() == "!comment" {
                        if !trimmed.starts_with('#') {
                            out.push_str(&format!("{}# {}\n", indent, trimmed));
                        } else {
                            out.push_str(line);
                            out.push('\n');
                        }
                    } else {
                        if let Some(rest) = trimmed.strip_prefix("# ") {
                            out.push_str(&format!("{}{}\n", indent, rest));
                        } else if let Some(rest) = trimmed.strip_prefix('#') {
                            out.push_str(&format!("{}{}\n", indent, rest));
                        } else {
                            out.push_str(line);
                            out.push('\n');
                        }
                    }
                    handled = true;
                    break;
                }
            } else if key.as_str() == "!replace" {
                if let Some(pos) = value.find("=>") {
                    let old = &value[..pos];
                    let new = &value[pos + 2..];
                    if line.contains(old) {
                        out.push_str(&line.replace(old, new));
                        out.push('\n');
                        handled = true;
                        break;
                    }
                }
            } else {
                let prefix = format!("{}:", key);
                if trimmed.starts_with(prefix.as_str()) {
                    let rest = &trimmed[prefix.len()..];
                    // Only match the exact key (not KEY_suffix), value follows a space.
                    if rest.starts_with(' ') || rest.is_empty() {
                        // Preserve any inline comment starting with " #".
                        let comment = rest.find(" #").map(|i| &rest[i..]).unwrap_or("");
                        out.push_str(&format!("{}{}: {}{}\n", indent, key, value, comment));
                        handled = true;
                        break;
                    }
                }
            }
        }

        if !handled {
            out.push_str(line);
            out.push('\n');
        }
    }

    out
}
