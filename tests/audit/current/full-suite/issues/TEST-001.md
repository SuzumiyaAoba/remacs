全テストを通常実行すると `coverage_tests::u_probe_eval14` が終了せず、共有 `PROBE_LOCK` を待つ後続の unit probe も進まなくなります。タイムアウトが無いため、全スイートを無人で完走できません。

HEAD: `29e7c960edec18a99107741c351d95444f8f6b18`、macOS/aarch64、debug build。

## 再現

```sh
cargo test --offline --lib coverage_tests::u_probe_eval14 -- --exact --nocapture
```

全 unit 実行では3分以上進捗が止まり、個別の同じテストも120秒でタイムアウトしました。外部から時間上限を付けて終了する必要があります。

`tests/probe_eval14.el` の各トップレベル式の直前にマーカーを出す診断コピーでは、`(ci (describe-buffer-bindings (current-buffer)))` の直前まで到達し、その呼び出しから戻りませんでした。同じ対話初期化条件で45秒上限の追試でも同じ位置で停止しました。

一方、初期状態の新規バッチプロセスで `describe-buffer-bindings` だけを呼ぶと GNU は約0.4秒、remacsは起動を含め約10秒で正常終了しました。したがって、先行する多数の probe が変更した状態に依存します。停止位置は確認済みですが、どの状態変更が原因かは未確定です。

## 影響と対処案

`src/lib.rs` の probe マクロが共有ロック中に全ファイルを評価するため、1本の停止が後続を遮断します。今回の全領域検査は未完了テストを個別プロセスに分け、時間上限付きで残りを実行して回避しました。

probe の状態汚染を切り分け、各 probe を独立・有限時間で実行できるようにする必要があります。未確定の runtime バグとして断定せず、まず全テスト実行を止める再現可能なテスト問題として記録します。

[src/lib.rs:23](https://github.com/SuzumiyaAoba/remacs/blob/29e7c960edec18a99107741c351d95444f8f6b18/src/lib.rs#L23), [tests/probe_eval14.el](https://github.com/SuzumiyaAoba/remacs/blob/29e7c960edec18a99107741c351d95444f8f6b18/tests/probe_eval14.el)

<!-- remacs-suite-id: TEST-001 -->
