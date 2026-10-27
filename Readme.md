# Cieos（xv6-riscvのRust版実装）

## xv6-riscv（オリジナル）
https://github.com/mit-pdos/xv6-riscv

## ディレクトリ構成

```text
cieos/
├── api/             kernelとuserで共通の実装
├── kernel/          カーネル本体
│   ├── dev/         デバイス。現在はconsole/uartのみ。virtioなども入れるべきかも。
│   ├── fs/          ファイルシステム関連
│   ├── proc/        プロセス関連。仮想メモリ、ロック、トラップなど
│   ├── user/        システムコールやELFの定義など
│   └── utils/       表示処理
├── user/            user空間で動かすプログラム
│   ├── bin/         init、sh、各種コマンド
│   ├── test/        テストプログラム
│   └── ulib/        共通ライブラリ
├── mkfs/            ビルドしたuserの内容をfsにまとめるためのスクリプト。このクレートのみホストで動かす。
├── docs/            学習時に使ったまとめ
├── build/           ビルド結果
└── Makefile         ビルドと起動の設定
```

## 動作方法
### Makefile
```
make build: kernelをビルドする
make fs: filesystemをビルドする。必要ならuserの中身も一緒にビルドを行う。
make run: 必要ならkernelとfsをビルドした上で、qemuを使ってcieosを立ち上げる
make gdb: gdb-serverを立ち上げ、デバッグ用のgdb-serverを待ち構える。
```
### その他
```
gdb-multiarch build/kernel.elf -q -ex "target remote localhost:10000" : 立ち上げたgdb-serverに接続する
Ctrl+A X : 立ち上げたqemuを終了する
```

## 依存パッケージ

Linux向け。パッケージ名はDebianのもの。

- Rust（`rustc`、Cargo）: Rust 2024対応版
- `build-essential`: Makeとホスト向けリンカー
- `binutils-riscv64-unknown-elf`: RISC-V向けアセンブラとリンカー
- `qemu-system-misc`: `qemu-system-riscv64`で起動
- `gdb-multiarch`: デバッグする場合

他に、Rustのターゲットを追加する必要がある。

```sh
rustup target add riscv64gc-unknown-none-elf
```

外部クレートは`regex = 1.12.4`。MakefileからCargoで取得・ビルドする。

## ライセンス
[Lisence.md](Lisence.md)
