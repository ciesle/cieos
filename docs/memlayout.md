# メモリレイアウト（物理）

0x00_001_000 : boot ROM

0x02_000_000 : CLINT

0x0C_000_000 : PLIC

0x10_000_000 : uart0

0x10_001_000 : virtio disk

0x80_000_000 : qemuのboot ROMはここにジャンプする
0x80_000_000 : entry.S

end カーネルページの割当エリア？

0x88_000_000 : PHYSTOP 

# メモリレイアウト（カーネル）
1ページ：4096byte = 2^12 = 0x1000

0x80_000_000 : KERNBASE
(ここからkernelのtext, bss)

stack0 : schedulerの使うスタック

end : kernel.ldで定義されており、kernel areaの最後

0x88_000_000 : PHYSTOP 
ここまでが物理メモリで、ダイレクトマッピングでアクセスできる
ここから仮想メモリで、ページテーブルを介してアクセスする


...
...
(kstack(1))
(guard page)
(kstack(0))
(guard page)

0x3_fff_ffe_000 : trampoline

0x4_000_000_000 : MAXVA


# メモリレイアウト（ユーザ）
1ページ：4096byte = 2^12 = 0x1000

text

original data, bss

fixed-size-stack

expandable heap


0x3_fff_ffd_000 : trapframe

0x3_fff_ffe_000 : trampoline

0x4_000_000_000 : MAXVA