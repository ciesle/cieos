# trapをuser spaceからkernelに移すためのコード。その後、userに帰る
# カーネルは、このコードを含んだ領域をカーネルとユーザで同じ仮想アドレスにマップする
# ページテーブルを切り替えても動くようになっている。


.section trampsec

.globl trampoline
.globl usertrap
trampoline:
.align 4

.globl uservec
uservec:
	# trap.rsがstvecをこのアドレスにセットする
	# user spaceのtrapはsupervisor modeのここから、
	# user page tableを使い始まる

	# user a0をsscratchにしまう
	csrw sscratch, a0

	# それぞれのプロセスは別々のp->trapframeエリアをもつが、
	# 同じ仮想アドレスにマップされる
	li a0, {TRAPFRAME}

	sd ra, 40(a0)
	sd sp, 48(a0)
	sd gp, 56(a0)
	sd tp, 64(a0)
	sd t0, 72(a0)
	sd t1, 80(a0)
	sd t2, 88(a0)
	sd s0, 96(a0)
	sd s1, 104(a0)
	sd a1, 120(a0)
	sd a2, 128(a0)
	sd a3, 136(a0)
	sd a4, 144(a0)
	sd a5, 152(a0)
	sd a6, 160(a0)
	sd a7, 168(a0)
	sd s2, 176(a0)
	sd s3, 184(a0)
	sd s4, 192(a0)
	sd s5, 200(a0)
	sd s6, 208(a0)
	sd s7, 216(a0)
	sd s8, 224(a0)
	sd s9, 232(a0)
	sd s10, 240(a0)
	sd s11, 248(a0)
	sd t3, 256(a0)
	sd t4, 264(a0)
	sd t5, 272(a0)
	sd t6, 280(a0)

	# user a0をp->trapframe->a0に保存する
	csrr t0, sscratch
	sd t0, 112(a0)

	# kernel stack pointerを初期化。
	# p->trapframe->kernel_spに入っている
	ld sp, 8(a0)

	# tpに今のhartidを保存させる
	ld tp, 32(a0)

	# usertrap()のアドレスをt0にロード
	ld t0, 16(a0)

	# kernel page tableのアドレスもロード
	ld t1, 0(a0)

	# 以前のメモリ操作がすべて終わるまで待つ。
	# user page tableを使っていると壊れるため
	sfence.vma zero, zero

	# kernel page tableをインストール
	csrw satp, t1

	# TLBから古いエントリを消す
	sfence.vma zero, zero

	# usertrap()を呼び出す
	# raにこの次の命令（userret）を登録
	jalr t0

.globl userret
userret:
	# usertrap()はここに返ってくる。user satpがa0に入っている


	# 初めてこのCPUでこのプロセスを実行する場合に備えて
	# i(nstruction)cacheをクリアする
	# 命令キャッシュが存在し、古い命令を読み込むことを避けるため
	fence.i

	# user page tableにスイッチする
	sfence.vma zero, zero
	csrw satp, a0
	sfence.vma zero, zero

	li a0, {TRAPFRAME}

	# a0以外を回復する
	ld ra, 40(a0)
	ld sp, 48(a0)
	ld gp, 56(a0)
	ld tp, 64(a0)
	ld t0, 72(a0)
	ld t1, 80(a0)
	ld t2, 88(a0)
	ld s0, 96(a0)
	ld s1, 104(a0)
	ld a1, 120(a0)
	ld a2, 128(a0)
	ld a3, 136(a0)
	ld a4, 144(a0)
	ld a5, 152(a0)
	ld a6, 160(a0)
	ld a7, 168(a0)
	ld s2, 176(a0)
	ld s3, 184(a0)
	ld s4, 192(a0)
	ld s5, 200(a0)
	ld s6, 208(a0)
	ld s7, 216(a0)
	ld s8, 224(a0)
	ld s9, 232(a0)
	ld s10, 240(a0)
	ld s11, 248(a0)
	ld t3, 256(a0)
	ld t4, 264(a0)
	ld t5, 272(a0)
	ld t6, 280(a0)

	# a0を回復する
	ld a0, 112(a0)

	# user mod, user pcに帰る
	# prepare_return()がsstatusとsepcを設定する
	sret