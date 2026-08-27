//! Generate the `MessageLabel` enum from the libsel4 interface XML files.
//!
//! This mirrors `kernel/tools/invocation_header_gen.py` and the three XML files
//! that both libsel4 and the kernel use to assign invocation labels, so the
//! kernel's labels always match user-space for every config combination
//! (mcs / smp / hypervisor / smc).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Map a `<config var="..."/>` name to the rel4 cargo feature that gates it.
/// `None` means rel4 does not support that config, so the condition is always
/// false and the method is omitted entirely.
fn config_var_to_feature(var: &str) -> Option<&'static str> {
    match var {
        "CONFIG_KERNEL_MCS" => Some("kernel_mcs"),
        "CONFIG_ENABLE_SMP_SUPPORT" => Some("enable_smp"),
        "CONFIG_ARM_HYPERVISOR_SUPPORT" => Some("hypervisor"),
        "CONFIG_ENABLE_SMC" => Some("enable_smc"),
        _ => None,
    }
}

/// A boolean condition over cargo features, mirroring the XML `<condition>`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cfg {
    True,
    False,
    Feature(&'static str),
    Not(Box<Cfg>),
    All(Vec<Cfg>),
    Any(Vec<Cfg>),
}

impl Cfg {
    /// Evaluate this condition against the set of enabled features.
    fn eval(&self, enabled: &BTreeSet<&str>) -> bool {
        match self {
            Cfg::True => true,
            Cfg::False => false,
            Cfg::Feature(f) => enabled.contains(f),
            Cfg::Not(inner) => !inner.eval(enabled),
            Cfg::All(items) => items.iter().all(|i| i.eval(enabled)),
            Cfg::Any(items) => items.iter().any(|i| i.eval(enabled)),
        }
    }

    /// Render this condition to a Rust `cfg(...)` expression for `#[cfg(...)]`.
    fn render(&self) -> Option<String> {
        match self {
            Cfg::True => Some(String::new()),
            Cfg::False => None,
            Cfg::Feature(f) => Some(format!("feature = \"{}\"", f)),
            Cfg::Not(inner) => match inner.render() {
                None => Some(String::new()),
                Some(s) if s.is_empty() => None,
                Some(s) => Some(format!("not({})", s)),
            },
            Cfg::All(items) => {
                let mut parts = Vec::new();
                for item in items {
                    match item.render() {
                        None => return None,
                        Some(s) if s.is_empty() => {}
                        Some(s) => parts.push(s),
                    }
                }
                Some(join_cfg(parts, "all"))
            }
            Cfg::Any(items) => {
                let mut parts = Vec::new();
                for item in items {
                    match item.render() {
                        None => {}
                        Some(s) if s.is_empty() => return Some(String::new()),
                        Some(s) => parts.push(s),
                    }
                }
                Some(join_cfg(parts, "any"))
            }
        }
    }
}

fn join_cfg(parts: Vec<String>, op: &str) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts.into_iter().next().unwrap(),
        _ => format!("{}({})", op, parts.join(", ")),
    }
}

fn parse_condition(node: roxmltree::Node) -> Cfg {
    match node.tag_name().name() {
        "config" => match node.attribute("var").and_then(config_var_to_feature) {
            Some(f) => Cfg::Feature(f),
            None => Cfg::False,
        },
        "not" => Cfg::Not(Box::new(parse_condition(
            node.first_element_child().expect("not has a child"),
        ))),
        "and" => Cfg::All(
            node.children()
                .filter(|c| c.is_element())
                .map(parse_condition)
                .collect(),
        ),
        "or" => Cfg::Any(
            node.children()
                .filter(|c| c.is_element())
                .map(parse_condition)
                .collect(),
        ),
        _ => Cfg::True,
    }
}

struct Label {
    name: String,
    cfg: Cfg,
}

fn parse_labels(xml_path: &Path) -> Result<Vec<Label>> {
    let text = fs::read_to_string(xml_path)
        .with_context(|| format!("failed to read {}", xml_path.display()))?;
    let doc = roxmltree::Document::parse(&text)
        .with_context(|| format!("failed to parse {}", xml_path.display()))?;

    let mut labels = Vec::new();
    for method in doc.root().descendants().filter(|n| n.has_tag_name("method")) {
        let name = method
            .attribute("id")
            .ok_or_else(|| anyhow!("method without id in {}", xml_path.display()))?
            .to_string();
        let cfg = method
            .children()
            .filter(|c| c.has_tag_name("condition"))
            .next()
            .and_then(|cond| cond.first_element_child())
            .map(parse_condition)
            .unwrap_or(Cfg::True);
        labels.push(Label { name, cfg });
    }
    Ok(labels)
}

/// Resolve an interface XML file, trying the source-tree layout first and the
/// flattened `ninja install` layout (`<prefix>/libsel4/include/interfaces/*.xml`)
/// second.
fn resolve_xml_file(
    libsel4_dir: &std::path::Path,
    source_subdir: &str,
    file: &str,
) -> std::path::PathBuf {
    let source = libsel4_dir.join(source_subdir).join(file);
    if source.exists() {
        source
    } else {
        libsel4_dir.join("interfaces").join(file)
    }
}

/// Generate the `MessageLabel` enum and a consistency check (方案 B).
pub fn generate_message_label(
    libsel4_dir: &Path,
    arch: &str,
    sel4_arch: &str,
    enabled_features: &[&str],
    out_path: &Path,
) -> Result<()> {
    let api = parse_labels(&resolve_xml_file(
        libsel4_dir,
        "include/interfaces",
        "object-api.xml",
    ))?;
    let sel4_arch_labels = parse_labels(&resolve_xml_file(
        libsel4_dir,
        &format!("sel4_arch_include/{sel4_arch}/interfaces"),
        "object-api-sel4-arch.xml",
    ))?;
    let arch_labels = parse_labels(&resolve_xml_file(
        libsel4_dir,
        &format!("arch_include/{arch}/interfaces"),
        "object-api-arch.xml",
    ))?;

    let enabled: BTreeSet<&str> = enabled_features.iter().copied().collect();

    let mut out = String::new();
    out.push_str("// Auto-generated by rel4_config::message_label_gen. Do not edit.\n");
    out.push_str("// Invocation labels mirror libsel4's invocation.h.\n");
    out.push_str("#[derive(Eq, PartialEq, Debug, Clone, Copy, PartialOrd, Ord)]\n");
    out.push_str("#[repr(C)]\n");
    out.push_str("pub enum MessageLabel {\n");
    out.push_str("    InvalidInvocation = 0,\n");

    let mut next_value: usize = 1;
    let mut checks = String::new();

    for label in api.into_iter().chain(sel4_arch_labels).chain(arch_labels) {
        if !label.cfg.eval(&enabled) {
            continue;
        }
        match label.cfg.render() {
            None => continue,
            Some(cfg) if cfg.is_empty() => {
                out.push_str(&format!("    {},\n", label.name));
                checks.push_str(&format!(
                    "const _: () = assert!(MessageLabel::{} as usize == {});\n",
                    label.name, next_value
                ));
            }
            Some(cfg) => {
                out.push_str(&format!("    #[cfg({})]\n    {},\n", cfg, label.name));
                checks.push_str(&format!(
                    "#[cfg({})]\nconst _: () = assert!(MessageLabel::{} as usize == {});\n",
                    cfg, label.name, next_value
                ));
            }
        }
        next_value += 1;
    }

    out.push_str("    nArchInvocationLabels,\n");
    out.push_str("}\n");

    // 方案 B: compile-time consistency check. `next_value` is the value the
    // compiler must assign to `nArchInvocationLabels`; if the enum and this
    // generator ever disagree (e.g. a feature/cfg mapping bug), the build fails.
    out.push_str("\n// Consistency check (方案 B).\n");
    out.push_str(&format!(
        "const _: () = assert!(MessageLabel::nArchInvocationLabels as usize == {});\n",
        next_value
    ));
    out.push_str(&checks);

    fs::write(out_path, &out)
        .with_context(|| format!("failed to write {}", out_path.display()))?;
    Ok(())
}
