OUTPUT_ARCH("riscv")
ENTRY(_entry)

MEMORY {
  ram (wxa!ri) : ORIGIN = 0x80000000, LENGTH = 128M
}

SECTIONS {
  . = 0x80000000; // 0x80_000_000

  .text : {
    build/entry.o(.text)
    *(.text .text.*)
    . = ALIGN(0x1000);
    _trampoline = .;
    *(trampsec)
    . = ALIGN(0x1000);
    ASSERT(. - _trampoline == 0x1000, "error: trampoline larger than one page");
    PROVIDE(etext = .);
  } >ram AT>ram

  .rodata : {
    . = ALIGN(16);
    *(.srodata .srodata.*)
    . = ALIGN(16);
    *(.rodata .rodata.*)
  } >ram AT>ram

  .data : {
    . = ALIGN(16);
    *(.sdata .sdata.*);
    . = ALIGN(16);
    *(.data .data.*);
  } >ram AT>ram
  
  .sbss : {
    . = ALIGN(16);
    *(.sbss .sbss.*)
    . = ALIGN(16);
    *(.bss .bss.*)
  } >ram AT>ram

  .stack0 : {
    . = ALIGN(16);
    KEEP(*(.stack0));
  } >ram AT>ram

  PROVIDE(end = .);
}