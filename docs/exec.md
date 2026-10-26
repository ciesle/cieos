# execの実行機序
init内のexec("/bin/sh", &argv)を例に考える

1. initからexec("/bin/sh", &argv)を呼び出す
2. usys::sys_execが呼び出され、a7に7をロードしてecallする
3. uservec→usertrap→scauseが8なのでsyscall→sys_exec
4. sys_execで引数を整える
5. kexecに渡る
   1. FSからelfを読み込む。
   2. 新しいページテーブルを作成
   3. Loadなセグメントをメモリ上に入れる
   4. スタック上に引数を配置する。そのポインタも配置する
   5. 第1引数にspを保存
   6. epc, spなどをtrapframeに設定
   7. ページテーブルを古いものと入れ替える
6. 第1引数を&argvとして、returnする
7. syscall → usertrap → userret → 
8. 返る先では新しいpagetableが設定されている。/bin/shのentry pointから動き出す

## kexit
1. sys_exitが呼び出される
2. 自分の子について、全てinitの養子に出す
3. 親をwakeupする
4. p.xstateに終了コードを格納する
5. p.stateをZOMBIEにする

## wait
1. プロセスを全件走査して、自分の子であり、ZOMBIEでもあるプロセスを探す
2. 見つけたら終了ステータスと子のpidを返す
3. 子があるが何も終わっていないとsleep