PLAT := qemu-arm-virt

all: build

build:
	cargo xtask build -p $(PLAT) -m off -s off

run: 
	cd build && ./simulate

.PHONY: all build
