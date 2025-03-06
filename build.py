#!/usr/bin/env python3
#
# Copyright 2020, Data61, CSIRO (ABN 41 687 119 230)
#
# SPDX-License-Identifier: BSD-2-Clause
#

import subprocess
import sys
import argparse
import time
import os
import shutil
from pygments import highlight
from pygments.lexers import BashLexer
from pygments.formatters import TerminalFormatter
import kernel.generator as gen

build_dir = "./build"

def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument('-b', '--baseline', dest="baseline", action="store_true",
                        help="baseline switch")
    # parser.add_argument('-a', '--arch', dest="architecture", default="riscv64", help="build architecture")
    parser.add_argument('-p', '--platform', dest='platform', default='spike', help="set-platform")
    parser.add_argument('-c', '--cpu', dest="cpu_nums", type=int,
                        help="kernel & qemu cpu nums", default=1)
    parser.add_argument('-m', '--mcs', dest='mcs', default='off', help="set-mcs")
    parser.add_argument('-s', '--smc', dest='smc', default='off', help="set-arm-smc-enable")
    parser.add_argument('-B', '--bin', dest='bin', action='store_true', help="use rel4 kernel binary")
    args = parser.parse_args()
    return args

def exec_shell(shell_command):
    ret_code = os.system(shell_command)
    return ret_code == 0

def clean_config():
    # shell_command = "cd ../kernel && git checkout 552f173d3d7780b33184ebedefc58329ea5de3ba"
    # exec_shell(shell_command)
    pass

if __name__ == "__main__":
    args = parse_args()
    clean_config()
    progname = sys.argv[0]

    rust_command = "cargo build --release"
    cmake_command = f"cd ./build && ../../init-build.sh  -DPLATFORM={args.platform} -DSIMULATION=TRUE"
    #TODO: later, call generator tools in cmake
    # generator_defs = ["CONFIG_HAVE_FPU", "CONFIG_FASTPATH"]

    if args.platform == "spike":
        rust_command += " --target riscv64imac-unknown-none-elf"
    elif args.platform == "qemu-arm-virt":
        rust_command += " --target aarch64-unknown-none-softfloat"
        if args.smc == "on":
            cmake_command += " -DKernelAllowSMCCalls=ON"
            rust_command += " --features ENABLE_SMC"
    
    if args.mcs == "on":
        rust_command += " --features KERNEL_MCS"
        cmake_command += " -DMCS=TRUE "
        # generator_defs.append("CONFIG_KERNEL_MCS")

    if args.cpu_nums > 1:
        rust_command += " --features ENABLE_SMP"
        cmake_command += " -DSMP=TRUE"

    if args.bin == False:
        rust_command += " --lib"
    else:
        rust_command += " --bin rel4_kernel --features BUILD_BINARY"
        cmake_command += " -DREL4_KERNEL=TRUE"
    
    cmake_command += " && ninja"
    
    # generator some code
    # gen.linker_gen(args.platform)
    # gen.dev_gen(args.platform)
    # default enable CONFIG_HAVE_FPU
    # gen.asms_gen(args.platform, generator_defs)

    if os.path.exists(build_dir):
        shutil.rmtree(build_dir)
    os.makedirs(build_dir)

    # prebuild rel4_kernel
    if args.baseline == True:
        shell_command = "cd ../kernel && git checkout baseline"
        if not exec_shell(shell_command):
            clean_config()
            sys.exit(-1)
    else:
        if not exec_shell(rust_command):
            clean_config()
            sys.exit(-1)
    
    # build sel4test
    if not exec_shell(cmake_command):
        clean_config()
        sys.exit(-1)
    clean_config()
