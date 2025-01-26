import yaml, os


def linker_gen(platform):
    src_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    config_file = os.path.join(src_dir, "kernel/src/platform", f"{platform}.yml")
    linker_file = os.path.join(src_dir, "kernel/src/arch/linker_gen.ld")
    with open(config_file, 'r') as file:
        doc = yaml.safe_load(file)
        kstart = doc['memory']['kernel_start']
        vmem_offset = doc['memory']['vmem_offset']
        arch = doc["cpu"]["arch"]
    
    with open(linker_file, 'w') as file:
        file.write("# This file is auto generated\n")
        file.write(f"OUTPUT_ARCH({arch})\n\n")
        file.write(f"KERNEL_OFFSET = {vmem_offset:#x};\n")
        file.write(f"START_ADDR = {(vmem_offset + kstart):#x};\n\n")
        file.write("INCLUDE kernel/src/arch/linker.ld.in")


def dev_gen(platform):
    src_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    config_file = os.path.join(src_dir, "kernel/src/platform", f"{platform}.yml")
    dev_file = os.path.join(src_dir, "kernel/src/platform/dev_gen.rs")
    with open(config_file, 'r') as file:
        doc = yaml.safe_load(file)
        avail_mem_zones = doc['memory']['avail_mem_zone']

    with open(dev_file, 'w') as file:
        # generate avail_p_regs
        file.write("// This file is auto generated\n")
        file.write("use crate::structures::p_region_t;\n\n")
        file.write("#[link_section = \".boot.bss\"]\n")
        file.write(f"pub static avail_p_regs: [p_region_t; {len(avail_mem_zones)}] = [\n")
        for zone in avail_mem_zones:
            file.write("    p_region_t {\n")
            file.write(f"       start: {zone['start']:#x},\n")
            file.write(f"       end: {zone['end']:#x}\n")
            file.write("    },\n")
        file.write("];\n")    

if __name__ == "__main__":
    linker_gen("qemu-arm-virt")
    dev_gen("qemu-arm-virt")
    