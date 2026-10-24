`pty_editor_smoke` は初期フレームが描画されていても、生の ANSI 出力中の `scratch` を見つけられずタイムアウトします。

HEAD `29e7c960edec18a99107741c351d95444f8f6b18`、macOS/aarch64 で、全 CLI テストは15成功・本テスト1失敗。約137秒後に `timed out waiting for the first rendered frame` となりました。

```sh
cargo test --offline --test main_cli pty_editor_smoke -- --exact
```

## 切り分け

同じバイナリを、サイズを明示した制御PTYで GNU Emacs と比較しました。

- 80x24: GNU/remacs とも初期フレームを確認でき、C-x C-c で終了コード0。
- 0x0: GNU/remacs とも ANSI を除去した出力では初期フレームを確認でき、C-x C-c で終了コード0。
- 0x0 の remacs 出力では `*scr` と `atch*` の間に ANSI リセットが入るため、生バイトの `scratch` は false になる。

実際の一部分:

```text
----- *scr\x1b[0match*   L1 C0  [sel]
```

テストは `script` が作るPTYのサイズを指定せず、`String::from_utf8_lossy(...).contains("scratch")` だけで最初のフレームを判定しています。この条件を描画不能と同一視すると偽の失敗になります。

端末サイズを固定した上で ANSI を解釈・除去して表示を確認するなど、実際のフレームを判定する必要があります。今回の追試ではエディタ自体の起動停止とは区別できました。

[tests/main_cli.rs:189](https://github.com/SuzumiyaAoba/remacs/blob/29e7c960edec18a99107741c351d95444f8f6b18/tests/main_cli.rs#L189)

<!-- remacs-suite-id: TEST-002 -->
