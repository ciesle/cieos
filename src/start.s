.align 2
.equ UART_BASE, 0x10000000
.equ REG_RBR, 0x00
.equ REG_THR, 0x00
.equ REG_IIR, 0x02
.equ LSR_RX_RDY, 0x01
.equ LSR_TX_RDY, 0x20

.section .text
.globl boot_entry
.globl stack_top

boot_entry:
	la sp, stack_top
	call main

loop:
	j loop