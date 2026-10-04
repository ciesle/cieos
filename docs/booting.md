# boot手順

## 重要ポイント
- swtchは、カーネル空間内でプロセス（spとra）を切り替える
- ecall -> uservec (-> usertrap -> hogefuga -> usertrap ->) userret -> sret で、カーネル/ユーザを切り替える。spやepcもすべて変わる

## 初めて
1. start.rsでmモードにおける設定を行う
2. main.rsでCPU0がinit類を呼び出す
3. schedule内でinitが選択される
   1. この中で、fsinitが行われる
4. swtchでinitのコンテキストを用いてforkretにreturnする。
   1. return addressを切り替える
   2. spを切り替える
   3. CPUのコンテキストにschedulerを保存
   4. procのコンテキストをロード
5. prepare_returnでsepc, trapvecなどを設定した後、userretに遷移
6. userretからsepcにreturn　（sepcにはプロセスの開始アドレスが入っている）
   1. mepc
7. 処理を行う

## タイマー割り込みがあった
1. uservecでレジスタを保存。ページ切り替えjalrでusertrapに入る
   1. raをuserretに設定
2. usertrapで、s_causeが8でなく、which_devが2なので、yield_cpuを呼び出す
3. stateをRUNNABLEに変更
4. schedを呼び、swtchでforkretに戻る
   1. CPUに保存していたコンテキストを復元
5. schedulerに戻る

### 戻る時
6. schedulerからswtchでschedに戻る
7. usertrapに戻る
8. prepare_returnでsepc, trapvecなどを設定
9. userretからsretによりcsrのsepcに戻る（trapframeに保存されていて、w_sepcで設定された）
10. userに戻る

## システムコールがあった
1. ecallでuservecに入り、usertrapに入る
2. r_scauseが8なので、epcを+4した後、syscallを実行する
   1. a7レジスタから番号を復元
   2. trapframe.a0に戻り値を設定
3. 実行終了後、prepare_returnでsepc, trapvecを設定。
4. userretからsretによりcsrのsepcに戻る
5. userに戻る

## カーネルモードで割り込みがあった
1. kernelvecに入り、spやraを保存する
2. kerneltrapを呼び出し、sepc, sstatusを保存
   1. タイマー割り込みなら、yield_cpuを呼び出し、shed -> swtch -> 別CPU
   2. デバイス割り込みなら、処理をして戻る
3. sepc, sstatusを復元
4. kernelvecに戻り、レジスタを復元し、sretでcsrのsepcに戻る

