OUTPUT_ARCH("riscv")
ENTRY(boot_entry)

SECTIONS {
	. = 0x80000000;

	.text : {
		*(.text)
	}

	. += 0x8000;
	stack_top = .;
	program_end = .;
}