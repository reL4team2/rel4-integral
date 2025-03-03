ARCH := riscv64

ifeq ($(ARCH), riscv64)
TARGET := riscv64imac-unknown-none-elf
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
	rustup target add riscv64imac-unknown-none-elf
	rustup component add rust-src
build: gen
	cargo build --release --target $(TARGET)
run: gen
	cargo build --release --target $(TARGET)
gen:
	python3 generator.py -p $(PLATFORM)
.PHONY: all build env run
