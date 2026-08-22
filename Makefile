ASM_SRCS := $(wildcard src/*.s)
ASM_OBJS := $(patsubst src/%.s,build/%.o,$(ASM_SRCS))
RUST_SRCS := $(wildcard src/*.rs)

LINK_ARGS := $(foreach obj,$(ASM_OBJS),\
	-C link-arg=$(abspath $(obj)))

TARGET := riscv64imac-unknown-none-elf

OUT_DIR := build
KERNEL := $(OUT_DIR)/main.elf
LINKER_SCRIPT := src/linker.ld

.PHONY: all
all: $(KERNEL)

build: $(KERNEL)

$(OUT_DIR)/%.o: src/%.s
	mkdir -p $(@D)
	riscv64-unknown-elf-gcc \
		 -nostdlib -nostartfiles -fno-builtin -march=rv64g -mabi=lp64d -fPIC \
		  -c $< \
		  -o $@

$(KERNEL): $(ASM_OBJS) $(RUST_SRCS) $(LINKER_SCRIPT)
	mkdir -p $(@D)
	rustc \
        --target riscv64imac-unknown-none-elf \
         -C linker=rust-lld \
         -C panic=abort \
        -C link-arg=-T$(abspath src/linker.ld) \
        $(LINK_ARGS) \
        -o $@ \
        src/main.rs

run: $(KERNEL)
	qemu-system-riscv64 -machine virt -cpu rv64 -nographic -bios none -kernel $<

clean:
	rm -f start.o $(KERNEL)