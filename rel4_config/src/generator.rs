use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub fn linker_gen(platform: &str) -> PathBuf {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    let kstart =
        crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.kernel_start")
            .expect("memory.kernel_start not set");
    let vmem_offset =
        crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.vmem_offset")
            .expect("memory.vmem_offset not set");
    let arch = crate::utils::get_value_from_yaml(&yaml_cfg.to_str().unwrap(), "cpu.arch")
        .expect("cpu.arch not set");

    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let linker_file = src_dir.join("linker_gen.ld");

    let mut file = File::create(linker_file.clone()).expect("Unable to create file");
    writeln!(file, "# This file is auto generated").expect("Unable to write to file");
    writeln!(file, "OUTPUT_ARCH({})", arch).expect("Unable to write to file");
    writeln!(file, "\nKERNEL_OFFSET = {:#x};", vmem_offset).expect("Unable to write to file");
    writeln!(file, "START_ADDR = {:#x};", vmem_offset + kstart).expect("Unable to write to file");
    writeln!(file, "\nINCLUDE kernel/src/arch/{}/linker.ld.in", arch)
        .expect("Unable to write to file");

    linker_file
}

pub fn platform_gen(platform: &str) -> PathBuf {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    let avail_mem_zones =
        crate::utils::get_zone_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.avail_mem_zone")
            .expect("memory.avail not set");

    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let dev_file = src_dir.join("platform_gen.rs");
    let mut file = File::create(&dev_file).expect("Unable to create file");
    writeln!(file, "// This file is auto generated").expect("Unable to write to file");
    writeln!(file, "use crate::structures::p_region_t;").expect("Unable to write to file");
    writeln!(file, "#[link_section = \".boot.bss\"]").expect("Unable to write to file");
    writeln!(
        file,
        "pub static avail_p_regs: [p_region_t; {}] = [",
        avail_mem_zones.len()
    )
    .expect("Unable to write to file");

    for zone in avail_mem_zones {
        writeln!(file, "    p_region_t {{").expect("Unable to write to file");
        writeln!(file, "       start: {:#x},", zone.start).expect("Unable to write to file");
        writeln!(file, "       end: {:#x}", zone.end).expect("Unable to write to file");
        writeln!(file, "    }},").expect("Unable to write to file");
    }

    writeln!(file, "];").expect("Unable to write to file");

    dev_file
        .canonicalize()
        .expect("Unable to get absolute path")
}

pub fn asm_gen(dir: &str, name: &str, inc_dir: Vec<&str>, defs: &Vec<String>, out: Option<&str>) {
    let src = format!("{}/{}", dir, name);
    let out = if let Some(o) = out {
        o.to_string()
    } else {
        format!("{}/{}", std::env::var("OUT_DIR").unwrap(), name)
    };
    let mut build_args = vec!["-E", "-P"];
    for i in inc_dir {
        build_args.push("-I");
        build_args.push(i);
    }
    build_args.push(&src);

    for d in defs.iter() {
        build_args.push(d);
    }

    build_args.push("-o");
    build_args.push(&out);

    let status = std::process::Command::new("cpp").args(&build_args).status();

    match status {
        Ok(s) if s.success() => println!("Successfully preprocessed assembly: {}", name),
        Ok(s) => panic!("gcc returned a non-zero status: {}", s),
        Err(e) => panic!("Failed to preprocess assembly: {}", e),
    }
}

// generate config.h and config.rs
pub fn config_gen(platform: &str, custom_defs: &Vec<String>) {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    let mut defs = crate::utils::get_all_defs(yaml_cfg.to_str().unwrap());
    for d in custom_defs {
        let mut split = d.split('=');
        let key = split.next().unwrap();
        let value = split.next();
        if let Some(v) = value {
            if v == "true" {
                defs.entry(key.to_string())
                    .and_modify(|e| *e = Some(String::new()))
                    .or_insert(Some(String::new()));
            } else {
                defs.entry(key.to_string())
                    .and_modify(|e| *e = Some(v.to_string()))
                    .or_insert(Some(v.to_string()));
            }
        } else {
            defs.entry(key.to_string())
                .and_modify(|e| *e = None)
                .or_insert(None);
        }
    }
    // TODO: replace definitions from defs
    // write defs into .h
    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let header_file = src_dir.join("config.h");
    let mut file = File::create(&header_file).expect("Unable to create file");
    writeln!(file, "// This file is auto generated").expect("Unable to write to file");

    for (key, value) in defs.clone() {
        if let Some(val) = value {
            writeln!(file, "#define CONFIG_{} {}", key, val).expect("Unable to write to file");
        } else {
            writeln!(file, "// CONFIG_{} not set", key).expect("Unable to write to file");
        }
    }

    // write defs into config.rs
    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let config_file = src_dir.join("config.rs");
    let mut file = File::create(&config_file).expect("Unable to create file");
    writeln!(file, "// This file is auto generated").expect("Unable to write to file");
    for (key, value) in defs {
        if let Some(val) = value {
            if val.is_empty() {
                writeln!(file, "pub const CONFIG_{}: bool = true;", key)
                    .expect("Unable to write to file");
            } else if let Ok(num) = val.parse::<usize>() {
                writeln!(file, "pub const CONFIG_{}: usize = {};", key, num)
                    .expect("Unable to write to file");
            } else if let Ok(num) = usize::from_str_radix(val.trim_start_matches("0x"), 16) {
                writeln!(file, "pub const CONFIG_{}: usize = {};", key, num)
                    .expect("Unable to write to file");
            } else {
                writeln!(file, "pub const CONFIG_{}: &str = \"{}\";", key, val)
                    .expect("Unable to write to file");
            }
        } else {
            writeln!(file, "// CONFIG_{} not set", key).expect("Unable to write to file");
        }
    }
}
