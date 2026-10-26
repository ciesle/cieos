RUSTC = rustc
CARGO = cargo
AS = riscv64-unknown-elf-as
LD = riscv64-unknown-elf-ld
QEMU = qemu-system-riscv64
QEMU_ARGS = \
	-machine virt \
	-nographic \
	-bios none \
	-m 128M \
	-kernel $(KERNEL_ELF) \
	-global virtio-mmio.force-legacy=false \
	-drive file=$(FS_IMG),if=none,format=raw,id=x0 \
	-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

OUT_DIR = build
TARGET = riscv64gc-unknown-none-elf

ASFLAGS = -march=rv64gc -mabi=lp64d
LDFLAGS = -T $(LINKER_SCRIPT)

# KERNEL Settings
KERNEL_DIR = kernel
KERNEL_ELF = $(OUT_DIR)/kernel.elf
KERNEL_LINKER = $(KERNEL_DIR)/kernel.ld

KERNEL_ASM_SRC = \
	$(KERNEL_DIR)/entry.s \
	$(KERNEL_DIR)/proc/kernelvec.s \
	$(KERNEL_DIR)/proc/swtch.s
KERNEL_ASM_OBJ = $(patsubst $(KERNEL_DIR)/%.s,$(OUT_DIR)/$(KERNEL_DIR)/%.o,$(KERNEL_ASM_SRC))

# Shared API crate
API_DIR = api
API_RLIB = $(OUT_DIR)/$(API_DIR)/libapi.rlib
API_HOST_RLIB = $(OUT_DIR)/host/$(API_DIR)/libapi.rlib

# USER Settings
USER_DIR = user
USER_LINKER = $(USER_DIR)/user.ld
ULIB_DIR = $(USER_DIR)/ulib
ULIB_RLIB = $(OUT_DIR)/$(ULIB_DIR)/libulib.rlib
USER_PROJECT_DIRS = $(filter-out $(ULIB_DIR)/% $(USER_DIR)/bin/grep/,$(wildcard $(USER_DIR)/*/*/))
USER_PROJECT_DIRS := $(foreach dir,$(USER_PROJECT_DIRS),\
	$(if $(wildcard $(dir)$(notdir $(patsubst %/,%,$(dir))).rs),$(dir)))
USER_PROJECTS = $(patsubst $(USER_DIR)/%/,%,$(USER_PROJECT_DIRS))
USER_ELFS = $(foreach project,$(USER_PROJECTS),$(OUT_DIR)/$(USER_DIR)/$(dir $(project))_$(notdir $(project)))

# Filesystem image
MKFS = $(OUT_DIR)/mkfs/mkfs
FS_IMG = $(OUT_DIR)/fs.img

RUSTFLAGS = \
	--verbose \
	--edition=2024 \
	--target $(TARGET) \
	-C panic=abort \
	-C opt-level=2 \
    -C debuginfo=2 \
	-C linker=$(LD)

USER_RUSTFLAGS = $(RUSTFLAGS) -C opt-level=z -C debuginfo=0 -C strip=symbols -C lto -C codegen-units=1
HOST_RUSTFLAGS = --edition=2024
KERNEL_LDFLAGS = -C link-arg=-T$(KERNEL_LINKER) $(foreach obj,$(KERNEL_ASM_OBJ),-C link-arg=$(obj))
USER_LDFLAGS = \
	-C relocation-model=static -C code-model=medium \
	-C link-arg=-T$(USER_LINKER) \
	-C link-arg=-estart -C link-arg=--no-relax \
	-C link-arg=-z -C link-arg=max-page-size=4096

CONFIG_DIR = $(OUT_DIR)/config
TARGET_CONFIG = $(CONFIG_DIR)/target.flags
HOST_CONFIG = $(CONFIG_DIR)/host.flags
KERNEL_CONFIG = $(CONFIG_DIR)/kernel.flags
USER_CONFIG = $(CONFIG_DIR)/user.flags
ASM_CONFIG = $(CONFIG_DIR)/asm.flags
REGEX_CONFIG = $(REGEX_DIR)/build.flags
FS_FILES = $(CONFIG_DIR)/fs-files

# External crates (Cargo is used only for these dependencies).
REGEX_DIR = $(OUT_DIR)/regex
REGEX_TARGET_DIR = $(REGEX_DIR)/target
REGEX_LIB_DIR = $(REGEX_TARGET_DIR)/$(TARGET)/release
REGEX_RLIB = $(REGEX_LIB_DIR)/libcieos_regex.rlib
REGEX_RUSTFLAGS = -C linker=$(LD) -C embed-bitcode=yes

define REGEX_MANIFEST
[package]
name = "cieos-regex"
version = "0.1.0"
edition = "2024"
[lib]
path = "lib.rs"
[dependencies]
regex = { version = "=1.12.4", default-features = false, features = ["unicode"] }
[profile.release]
opt-level = "z"
debug = false
codegen-units = 1
panic = "abort"
[workspace]
endef

define REGEX_SOURCE
#![no_std]
pub use regex::*;
endef

build: $(KERNEL_ELF)

# Change timestamps only when the effective options or filesystem inputs change.
$(TARGET_CONFIG): export BUILD_OPTIONS = $(RUSTC) $(RUSTFLAGS)
$(HOST_CONFIG): export BUILD_OPTIONS = $(RUSTC) $(HOST_RUSTFLAGS)
$(KERNEL_CONFIG): export BUILD_OPTIONS = -A unused $(KERNEL_LDFLAGS)
$(USER_CONFIG): export BUILD_OPTIONS = $(RUSTC) $(USER_RUSTFLAGS) $(USER_LDFLAGS)
$(ASM_CONFIG): export BUILD_OPTIONS = $(AS) $(ASFLAGS)
$(REGEX_CONFIG): export BUILD_OPTIONS = $(CARGO) $(RUSTC) $(REGEX_RUSTFLAGS) $(TARGET)
$(FS_FILES): export BUILD_OPTIONS = $(USER_ELFS)
$(REGEX_DIR)/Cargo.toml: export BUILD_OPTIONS = $(REGEX_MANIFEST)
$(REGEX_DIR)/lib.rs: export BUILD_OPTIONS = $(REGEX_SOURCE)

$(TARGET_CONFIG) $(HOST_CONFIG) $(KERNEL_CONFIG) $(USER_CONFIG) $(ASM_CONFIG) $(REGEX_CONFIG) $(FS_FILES) $(REGEX_DIR)/Cargo.toml $(REGEX_DIR)/lib.rs: FORCE
	@mkdir -p $(@D)
	@printf '%s\n' "$$BUILD_OPTIONS" > $@.tmp
	@if cmp -s $@.tmp $@; then rm -f $@.tmp; else mv $@.tmp $@; fi

$(REGEX_RLIB): $(REGEX_DIR)/Cargo.toml $(REGEX_DIR)/lib.rs $(REGEX_CONFIG)
	RUSTC=$(RUSTC) RUSTFLAGS='$(REGEX_RUSTFLAGS)' $(CARGO) build \
		--manifest-path $(REGEX_DIR)/Cargo.toml \
		--target-dir $(REGEX_TARGET_DIR) --target $(TARGET) --release
	touch $@

$(API_RLIB): $(API_DIR)/lib.rs $(TARGET_CONFIG)
	mkdir -p $(@D)
	$(RUSTC) $(RUSTFLAGS) --emit=link,dep-info=$@.d --crate-name api --crate-type=rlib $< -o $@

$(API_HOST_RLIB): $(API_DIR)/lib.rs $(HOST_CONFIG)
	mkdir -p $(@D)
	$(RUSTC) $(HOST_RUSTFLAGS) --emit=link,dep-info=$@.d --crate-name api --crate-type=rlib $< -o $@

$(KERNEL_ELF): $(KERNEL_DIR)/main.rs $(KERNEL_ASM_OBJ) $(KERNEL_LINKER) $(API_RLIB) $(TARGET_CONFIG) $(KERNEL_CONFIG)
	mkdir -p $(@D)
	$(RUSTC) $(RUSTFLAGS) --emit=link,dep-info=$@.d \
		-A unused \
		--extern api=$(API_RLIB) \
		$(KERNEL_LDFLAGS) \
		$(KERNEL_DIR)/main.rs -o $@

$(ULIB_RLIB): $(ULIB_DIR)/lib.rs $(API_RLIB) $(USER_CONFIG)
	mkdir -p $(@D)
	$(RUSTC) $(USER_RUSTFLAGS) --emit=link,dep-info=$@.d --crate-name ulib --crate-type=rlib \
		--extern api=$(API_RLIB) $(ULIB_DIR)/lib.rs -o $@

define USER_PROGRAM_RULE
$(OUT_DIR)/$(USER_DIR)/$(dir $(1))_$(notdir $(1)): $(USER_DIR)/$(1)/$(notdir $(1)).rs $(USER_LINKER) $(ULIB_RLIB) $(API_RLIB) $(USER_CONFIG)
endef
$(foreach project,$(USER_PROJECTS),$(eval $(call USER_PROGRAM_RULE,$(project))))

$(OUT_DIR)/$(USER_DIR)/bin/_wc $(OUT_DIR)/$(USER_DIR)/bin/_grep: $(REGEX_RLIB)
$(OUT_DIR)/$(USER_DIR)/bin/_wc $(OUT_DIR)/$(USER_DIR)/bin/_grep: private USER_RUSTFLAGS += --extern regex=$(REGEX_RLIB) -L dependency=$(REGEX_LIB_DIR)/deps

$(USER_ELFS):
	mkdir -p $(@D)
	$(RUSTC) $(USER_RUSTFLAGS) --emit=link,dep-info=$@.d --crate-name $(patsubst _%,%,$(@F)) --crate-type=bin \
		--extern ulib=$(ULIB_RLIB) --extern api=$(API_RLIB) \
		-L dependency=$(dir $(API_RLIB)) \
		$(USER_LDFLAGS) \
		$< -o $@

$(KERNEL_ASM_OBJ): $(OUT_DIR)/$(KERNEL_DIR)/%.o: $(KERNEL_DIR)/%.s $(ASM_CONFIG)
	mkdir -p $(@D)
	$(AS) $(ASFLAGS) -o $@ $<

$(MKFS): mkfs/mkfs.rs $(API_HOST_RLIB) $(HOST_CONFIG)
	mkdir -p $(@D)
	$(RUSTC) $(HOST_RUSTFLAGS) --emit=link,dep-info=$@.d --extern api=$(API_HOST_RLIB) $< -o $@

$(FS_IMG): $(MKFS) $(USER_ELFS) $(FS_FILES)
	cd $(OUT_DIR) && mkfs/mkfs fs.img $(patsubst $(OUT_DIR)/%,%,$(USER_ELFS))

ifneq ($(filter clean,$(MAKECMDGOALS)),)
fs:
else
fs: user $(FS_IMG)
endif

user: $(USER_ELFS)

run qemu: build $(FS_IMG)
	$(QEMU) $(QEMU_ARGS) -smp 2

# gdb-multiarch build/kernel.elf -q \
  -ex "target remote localhost:10000"
gdb qemu-gdb: build $(FS_IMG)
	$(QEMU) $(QEMU_ARGS) \
		-S \
		-gdb tcp::10000

clean:
ifneq ($(filter fs,$(MAKECMDGOALS)),)
	rm -f $(FS_IMG)
else
	rm -rf $(filter-out $(REGEX_DIR),$(wildcard $(OUT_DIR)/*))
endif

.PHONY: build user fs run qemu gdb qemu-gdb clean FORCE

FORCE:

-include $(addsuffix .d,$(API_RLIB) $(API_HOST_RLIB) $(ULIB_RLIB) $(KERNEL_ELF) $(MKFS) $(USER_ELFS))
