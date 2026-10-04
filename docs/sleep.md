# Sleepの順番

↑ 判定リソースへのアクセスを制限
｜
｜判定
｜（waitの場合、ZOMBIEである子プロセスがあるか？）
｜
｜sleep_prepare
｜（chanを登録する）
｜
↓

↑対象プロセスのロックを取得
｜
｜sleep
｜（chanがあれば、schedを呼び出す）
｜
｜


1. sleep_prepareとsleepが同じ関数に入っている
   1. sleep以前にロックを解放：間で呼び出されると死
   2. sleepでロックを解除：sleepにロックを渡す必要がある
2. 2つに分ける
   1. プロセスに判定用情報を登録する
   2. shed() （判定用情報がなければ、すでにwakeupされているので、shedしない）