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
KERNEL_ASM_OBJ = $(patsubst $(KERNEL_DIR)/%.s,$(OUT_DIR)/$(KERNEL_DIR)/%.o,$(KERNEL_ASM_SRC))

# USER Settings
USER_DIR = user
USER_LINKER = $(USER_DIR)/user.ld
USER_MAIN = $(USER_DIR)/iocheck.rs
USER_ELF = $(OUT_DIR)/$(USER_DIR)/iocheck.elf
USER_RUST_SRC = $(wildcard $(USER_DIR)/*.rs)
USER_ASM_SRC = $(USER_DIR)/usys.s

RUSTFLAGS = \
	--target $(TARGET) \
	-C panic=abort \
    -C debuginfo=2 \
	-C linker=$(LD)

build: $(KERNEL_ELF) 

$(KERNEL_ELF): $(KERNEL_RUST_SRC) $(KERNEL_ASM_OBJ) $(KERNEL_LINKER)
	mkdir -p $(@D)
	$(RUSTC) $(RUSTFLAGS) \
		-A unused \
		-C link-arg=-T$(KERNEL_LINKER) \
		$(foreach obj,$(KERNEL_ASM_OBJ),-C link-arg=$(obj)) \
		$(KERNEL_DIR)/main.rs -o $@

$(USER_ELF): $(USER_RUST_SRC) $(KERNEL_ASM_SRC) $(USER_LINKER)
	mkdir -p $(@D)
	$(RUSTC) $(RUSTFLAGS) --edition=2024 --crate-type=bin \
		-C opt-level=2 \
		-C relocation-model=static -C code-model=medium \
		-C link-arg=-T$(USER_LINKER) \
		-C link-arg=-emain -C link-arg=--no-relax \
		-C link-arg=-z -C link-arg=max-page-size=4096 \
		$(USER_MAIN) -o $@

$(OUT_DIR)/$(KERNEL_DIR)/%.o: $(KERNEL_DIR)/%.s
	mkdir -p $(@D)
	$(AS) $(ASFLAGS) -o $@ $<

run: build
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-smp 2 \
		-kernel $(KERNEL_ELF)

# gdb-multiarch build/kernel.elf -q \
  -ex "target remote localhost:10000"
gdb: build
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