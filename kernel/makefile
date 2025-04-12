ARCH := riscv64
MCS := off

ifeq ($(ARCH), riscv64)
TARGET := riscv64gc-unknown-none-elf
PLATFORM	:= spike
else ifeq ($(ARCH), aarch64)
TARGET := aarch64-unknown-none-softfloat
PLATFORM	:= qemu-arm-virt
endif

all: build 
	@echo $(ARCH)
env:
	rustup install nightly-2023-05-01
	rustup default nightly-2023-05-01
	rustup target add riscv64gc-unknown-none-elf
	rustup component add rust-src
build:
	cargo xtask build -p ${PLATFORM} -m $(MCS) --rust-only
run:
	cargo xtask build -p ${PLATFORM} -m $(MCS) --rust-only
.PHONY: all build env run
