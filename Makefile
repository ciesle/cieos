RUSTC = rustc
AS = riscv64-unknown-elf-as
LD = riscv64-unknown-elf-ld
QEMU = qemu-system-riscv64

OUT_DIR = build
TARGET = riscv64gc-unknown-none-elf

ASFLAGS = -march=rv64gc -mabi=lp64d
LDFLAGS = -T $(LINKER_SCRIPT)

# KERNEL Settings
KERNEL_DIR = kernel
KERNEL_ELF = $(OUT_DIR)/kernel.elf
KERNEL_LINKER = $(KERNEL_DIR)/kernel.ld

KERNEL_RUST_SRC = $(wildcard $(KERNEL_DIR)/*.rs)
KERNEL_ASM_SRC = $(wildcard $(KERNEL_DIR)/*.s)
KERNEL_ASM_OBJ = $(patsubst $(KERNEL_DIR)/%.s,$(OUT_DIR)/%.o,$(KERNEL_ASM_SRC))

RUSTFLAGS = \
	--target $(TARGET) \
	-C panic=abort \
    -C debuginfo=2 \
	-C linker=$(LD)

build: $(KERNEL_ELF)

$(KERNEL_ELF): $(KERNEL_RUST_SRC) $(KERNEL_ASM_OBJ) $(KERNEL_LINKER)
	mkdir -p $(OUT_DIR)
	$(RUSTC) $(RUSTFLAGS) \
		-A unused \
		-C link-arg=-T$(KERNEL_LINKER) \
		$(foreach obj,$(KERNEL_ASM_OBJ),-C link-arg=$(obj)) \
		$(KERNEL_DIR)/main.rs -o $@

$(OUT_DIR)/%.o: $(KERNEL_DIR)/%.s
	mkdir -p $(OUT_DIR)
	$(AS) $(ASFLAGS) -o $@ $<

run: build
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-kernel $(KERNEL_ELF)

gdb: 
	$(QEMU) \
		-machine virt \
		-nographic \
		-machine virt \
		-m 128M \
		-kernel $(KERNEL_ELF) \
		-bios none \
		-S \
		-gdb tcp::10000

clean:
	rm -rf $(OUT_DIR)

.PHONY: build run gdb clean