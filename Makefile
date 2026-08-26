RUSTC = rustc
AS = riscv64-unknown-elf-as
LD = riscv64-unknown-elf-ld
QEMU = qemu-system-riscv64

TARGET = riscv64gc-unknown-none-elf

SRC_DIR = src
OUT_DIR = build

ELF = $(OUT_DIR)/main.elf
LINKER_SCRIPT = $(SRC_DIR)/linker.ld

ASFLAGS = -march=rv64gc -mabi=lp64d
LDFLAGS = -T $(LINKER_SCRIPT)

RUST_SRC = $(wildcard $(SRC_DIR)/*.rs)
ASM_SRC = $(wildcard $(SRC_DIR)/*.s)
ASM_OBJ = $(patsubst $(SRC_DIR)/%.s,$(OUT_DIR)/%.o,$(ASM_SRC))

RUSTFLAGS = \
	--target $(TARGET) \
	-C panic=abort \
	-C linker=$(LD) \
	-C link-arg=-T$(LINKER_SCRIPT)

build: $(ELF)

$(ELF): $(RUST_SRC) $(ASM_OBJ) $(LINKER_SCRIPT)
	mkdir -p $(OUT_DIR)
	$(RUSTC) $(RUSTFLAGS) \
		$(foreach obj,$(ASM_OBJ),-C link-arg=$(obj)) \
		$(SRC_DIR)/main.rs -o $@

$(OUT_DIR)/%.o: $(SRC_DIR)/%.s
	mkdir -p $(OUT_DIR)
	$(AS) $(ASFLAGS) -o $@ $<

run: build
	$(QEMU) \
		-machine virt \
		-nographic \
		-bios none \
		-kernel $(ELF)

clean:
	rm -rf $(OUT_DIR)

.PHONY: build run clean