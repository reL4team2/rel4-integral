import yaml, os
import subprocess
import argparse


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("-p", "--platform", dest="platform", help="target platform")
    parser.add_argument(
        "-d", "--define", dest="definitions", action="append", help="Macro Definitions"
    )
    args = parser.parse_args()
    return args


def linker_gen(platform):
    print(platform)
    src_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    config_file = os.path.join(src_dir, "kernel/src/platform", f"{platform}.yml")
    with open(config_file, "r") as file:
        doc = yaml.safe_load(file)
        kstart = doc["memory"]["kernel_start"]
        vmem_offset = doc["memory"]["vmem_offset"]
        arch = doc["cpu"]["arch"]

    linker_file = os.path.join(src_dir, f"kernel/src/arch/{arch}/linker_gen.ld")
    print(linker_file)
    with open(linker_file, "w") as file:
        file.write("# This file is auto generated\n")
        file.write(f"OUTPUT_ARCH({arch})\n\n")
        file.write(f"KERNEL_OFFSET = {vmem_offset:#x};\n")
        file.write(f"START_ADDR = {(vmem_offset + kstart):#x};\n\n")
        file.write(f"INCLUDE kernel/src/arch/{arch}/linker.ld.in")


def dev_gen(platform):
    src_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    config_file = os.path.join(src_dir, "kernel/src/platform", f"{platform}.yml")
    dev_file = os.path.join(src_dir, "kernel/src/platform/dev_gen.rs")
    with open(config_file, "r") as file:
        doc = yaml.safe_load(file)
        avail_mem_zones = doc["memory"]["avail_mem_zone"]

    with open(dev_file, "w") as file:
        # generate avail_p_regs
        file.write("// This file is auto generated\n")
        file.write("use rel4_arch::paddr;\n")
        file.write("use rel4_arch::basic::PRegion;\n\n")
        file.write('#[link_section = ".boot.bss"]\n')
        file.write(f"pub static avail_p_regs: [PRegion; {len(avail_mem_zones)}] = [\n")
        for zone in avail_mem_zones:
            file.write(
                f"    PRegion::new(paddr!({zone['start']:#x}), paddr!({zone['end']:#x})), \n"
            )
        file.write("];\n")


def asm_gen(platform, asm_name, config=[]):
    src_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    config_file = os.path.join(src_dir, "kernel/src/platform", f"{platform}.yml")
    arch = "riscv"
    with open(config_file, "r") as file:
        doc = yaml.safe_load(file)
        arch = doc["cpu"]["arch"]
        stack_bits = doc["memory"]["stack_bits"]

    asm_src_file = os.path.join(src_dir, f"kernel/src/arch/{arch}/{asm_name}")
    asm_gen_file = os.path.join(src_dir, f"kernel/src/arch/{arch}/gen/{asm_name}")
    include_dir = os.path.join(src_dir, f"kernel/include")
    if os.path.exists(os.path.dirname(asm_gen_file)) is False:
        os.makedirs(os.path.dirname(asm_gen_file))

    commands = ["gcc", "-E", f"-I{include_dir}"]
    commands.append(f"-DCONFIG_KERNEL_STACK_BITS={stack_bits}")
    for cfg in config:
        commands.append(f"-D{cfg}")
    commands.extend([asm_src_file, "-o", asm_gen_file])

    try:
        subprocess.run(commands, check=True)
    except subprocess.CalledProcessError as e:
        print(f"asm file generator error: {e}")


def asms_gen(platform, config=[]):
    asm_gen(platform, "traps.S", config)
    asm_gen(platform, "head.S", config)


if __name__ == "__main__":
    args = parse_args()
    defines = []
    if args.definitions is not None:
        for d in args.definitions:
            defines.append(d)

    linker_gen(args.platform)
    dev_gen(args.platform)
    asms_gen(args.platform, defines)
