use std::fs::File;
use std::path::PathBuf;
use std::io::Write;

pub fn linker_gen(platform: &str) -> PathBuf {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    let kstart = crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.kernel_start").expect("memory.kernel_start not set");
    let vmem_offset = crate::utils::get_int_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.vmem_offset").expect("memory.vmem_offset not set");
    let arch = crate::utils::get_value_from_yaml(&yaml_cfg.to_str().unwrap(), "cpu.arch").expect("cpu.arch not set");

    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let linker_file = src_dir.join("linker_gen.ld");

    let mut file = File::create(linker_file.clone()).expect("Unable to create file");
    writeln!(file, "# This file is auto generated").expect("Unable to write to file");
    writeln!(file, "OUTPUT_ARCH({})", arch).expect("Unable to write to file");
    writeln!(file, "\nKERNEL_OFFSET = {:#x};", vmem_offset).expect("Unable to write to file");
    writeln!(file, "START_ADDR = {:#x};", vmem_offset + kstart).expect("Unable to write to file");
    writeln!(file, "\nINCLUDE kernel/src/arch/{}/linker.ld.in", arch).expect("Unable to write to file");

    linker_file
}

pub fn platform_gen(platform: &str) -> PathBuf {
    let yaml_cfg = crate::utils::get_root().join(format!("cfg/platform/{}.yml", platform));
    let avail_mem_zones = crate::utils::get_zone_from_yaml(&yaml_cfg.to_str().unwrap(), "memory.avail_mem_zone").expect("memory.avail not set");

    let src_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| "target".into()));
    let dev_file = src_dir.join("platform_gen.rs");
    let mut file = File::create(&dev_file).expect("Unable to create file");
    writeln!(file, "// This file is auto generated").expect("Unable to write to file");
    writeln!(file, "use crate::structures::p_region_t;").expect("Unable to write to file");
    writeln!(file, "#[link_section = \".boot.bss\"]").expect("Unable to write to file");
    writeln!(file, "pub static avail_p_regs: [p_region_t; {}] = [", avail_mem_zones.len()).expect("Unable to write to file");

    for zone in avail_mem_zones {
        writeln!(file, "    p_region_t {{").expect("Unable to write to file");
        writeln!(file, "       start: {:#x},", zone.start).expect("Unable to write to file");
        writeln!(file, "       end: {:#x}", zone.end).expect("Unable to write to file");
        writeln!(file, "    }},").expect("Unable to write to file");
    }

    writeln!(file, "];").expect("Unable to write to file");

    dev_file.canonicalize().expect("Unable to get absolute path")
}

pub fn asm_gen(dir: &str, name: &str, inc_dir: &str, defs: &Vec<String>) {
    let src = format!("{}/{}", dir, name);
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out = format!("{}/{}", out_dir, name);
    let inc_param = format!("-I{}", inc_dir);

    let mut build_args = vec![
        "-E",
        &inc_param,
        &src,
    ];

    for d in defs.iter() {
        build_args.push(d);
    }

    build_args.push("-o");
    build_args.push(&out);

    let status = std::process::Command::new("gcc")
        .args(&build_args)
        .status();

    match status {
        Ok(s) if s.success() => println!("Successfully preprocessed assembly: {}", name),
        Ok(s) => eprintln!("gcc returned a non-zero status: {}", s),
        Err(e) => eprintln!("Failed to preprocess assembly: {}", e),
    }
}