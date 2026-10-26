.global sys_fork
sys_fork:
    li a7, 1
    ecall
    ret

.global sys_exit
sys_exit:
    li a7, 2
    ecall
    ret

.global sys_wait
sys_wait:
    li a7, 3
    ecall
    ret

.global sys_pipe
sys_pipe:
    li a7, 4
    ecall
    ret

.global sys_read
sys_read:
    li a7, 5
    ecall
    ret

.global sys_kill
sys_kill:
    li a7, 6
    ecall
    ret

.global sys_exec
sys_exec:
    li a7, 7
    ecall
    ret

.global sys_fstat
sys_fstat:
    li a7, 8
    ecall
    ret

.global sys_chdir
sys_chdir:
    li a7, 9
    ecall
    ret

.global sys_dup
sys_dup:
    li a7, 10
    ecall
    ret

.global sys_getpid
sys_getpid:
    li a7, 11
    ecall
    ret

.global sys_sbrk
sys_sbrk:
    li a7, 12
    ecall
    ret

.global sys_pause
sys_pause:
    li a7, 13
    ecall
    ret

.global sys_uptime
sys_uptime:
    li a7, 14
    ecall
    ret

.global sys_open
sys_open:
    li a7, 15
    ecall
    ret

.global sys_write
sys_write:
    li a7, 16
    ecall
    ret
.global sys_mknod
sys_mknod:
    li a7, 17
    ecall
    ret

.global sys_unlink
sys_unlink:
    li a7, 18
    ecall
    ret

.global sys_link
sys_link:
    li a7, 19
    ecall
    ret

.global sys_mkdir
sys_mkdir:
    li a7, 20
    ecall
    ret

.global sys_close
sys_close:
    li a7, 21
    ecall
    ret

.global sys_sync
sys_sync:
    li a7, 22
    ecall
    ret