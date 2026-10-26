# 構成と依存境界

GNU Emacs との観測可能な互換性を維持しながら、変更箇所と障害の切り分けを明確にする。
処理系・端末・GUI は別の Cargo パッケージとしてビルドできる。

```text
remacs（統合実行ファイル・既存 API）
 ├─ remacs-gui ── remacs-cli（terminal feature なし）
 │      └────────────┐
 ├─ remacs-cli ──────┤
 └─ remacs-runtime ◀─┘
          └─ remacs-libraries
```

## 処理系: crates/runtime

`lisp/` は reader・値・評価器・bytecode・ローダー・組み込み関数を保持する。
`buffer/` と `editor/` は Emacs Lisp から見えるバッファ、フレーム、ウィンドウ、
キーマップの状態とプリミティブであり、画面や端末デバイスを開かない。
GNU Emacs の Lisp API はこれらの状態を直接扱うため、処理系の互換実行環境に含める。
この整理ではそれらを削った別の Lisp 方言や、不完全な最小ブートを導入しない。

### Lisp 評価器の内部構成

`lisp/eval.rs` は `Interp` の状態、初期化、評価・関数呼び出しのディスパッチを保持する。
補助処理は `lisp/eval/` の非公開モジュールに分け、既存の `eval::*` 公開パスを再公開で維持する。

- `lexenv.rs`: 字句環境の永続的なリスト構造、共有束縛セル、検索・代入・動的宣言。
- `bindings.rs`: 動的束縛スタック、バッファローカル／デフォルト値の保存と復元、巻き戻し。
- `conditions.rs`: signal の生成、診断、条件名の照合。signal の配送は評価器に残す。
- `plist.rs`: シンボルプロパティで使う property list の検索と変更。

最適化時も、字句環境のセル共有と最初の一致で検索を止める規則、watcher の通知順、
巻き戻し中に発生した signal の配送順を維持する。状態を複製した別の実行経路は作らない。
束縛と条件の変更には `qa_eval`、`qa_eval_lexenv`、`qa_closure_constructor`、
`qa_variable_watchers`、`qa_local_dynamic_unwind`、`qa_signal_*`、`qa_plists` を使い、
通常起動と `REMACS_NO_ELC=1` のソース起動の両方で確認する。

macOS のプロセス属性取得と列挙は `lisp/process/macos.rs` と `macos.c` が所有する。
SDK の `kinfo_proc` などの構造体は C 側で扱い、固定幅のデータを Rust へ渡す。
`crates/runtime/build.rs` は build dependency の `cc` でこの橋渡しをコンパイルし、
source root とビルド情報の設定はルートの `build.rs` と共有する。

`frontend/` はフロントエンド共通の部分を持つ。

- `mod.rs`: `KeyIo` 入出力契約と公開 API。
- `render.rs`: face 解決と `Grid` / `CellStyle` による画面スナップショット。
- `command.rs`: コマンド実行、バッチループ、minibuffer の入力ループ。
- `command/isearch.rs`: インクリメンタル検索の状態と overlay。
- `tests.rs`: 実デバイスを使わない入力ループの回帰テスト。

`host.rs` は CLI が必要とするネイティブ起動操作、例外・quit の巻き戻しの境界。
CLI から組み込み関数の非公開モジュールへ直接依存しない。
公開 API を変える場合は `Interp` の内部フィールドを公開する前にここを検討する。

`remacs-lisp` は独立した実行ファイル。Lisp の評価、ファイル、標準入力を扱い、
GNU 互換の CLI オプションや端末初期化に依存しない。評価は大きなスタックを持つ
ワーカースレッドで行う。`Interp` は `!Send` のままそのスレッド内で生成する。

## CLI: crates/cli

`arguments.rs` がオプションと引数の境界を定義する。
`application.rs` が起動モード、イメージ復元、実行スレッドとフロントエンドを選ぶ。
GUI の起動関数は外側から渡し、このクレートから GUI には依存しない。

`startup/mod.rs` は Lisp 側の変更を反映する引数キューを順番に処理する。
`actions.rs` は評価とロード、`shutdown.rs` は終了・cleanup、`diagnostics.rs` は
エラー・バックトレースを担当する。評価順、Lisp の entry frame、終了フックを維持する。

`terminal.rs` が crossterm と ANSI 出力を所有する。
`terminal` feature を無効にすれば端末アダプターをリンクせずに起動処理を利用できる。
`remacs-cli` バイナリはこの feature を必要とする。

## GUI: crates/gui

`lib.rs` はアプリケーションの公開起動 API。
`logic.rs` は評価スレッドと、入力イベント／最新画面のチャネルを所有する。
`keys.rs` はコマンドキーを Emacs イベントへ変換する。
`view.rs` が GPUI のレイアウトと描画を行い、`view/input.rs` が IME と UTF-16 の
OS テキスト入力を扱う。確定前の入力は view、確定したテキストは評価スレッドが所有する。

GUI も CLI と同じ起動処理を使うが、GUI 単独ビルドでは crossterm に依存しない。
統合バイナリは GUI と端末の両方を提供し、`--no-default-features` で GUI を除外できる。

## 同梱 Lisp: crates/libraries

`lisp/` と `lisp-elc/` は従来のフラットな配置を保ち、内容を変更しない。
このデータ専用クレートが `EMBEDDED_LISP` と AOT 登録表を所有し、Rust の実行ロジックに
依存しない。処理系のローダーが検索順、ユーザーの load-path、ソースの実体化、
load-history、AOT／ソースの切り替えを担当する。

`tools/gen_embedded.py` と `tools/gen_boot_elc.py` はこのクレートを対象にする。
ブート用 `prelude.el` と GNU の所有権スナップショットは処理系実装に属する。
`REMACS_SOURCE_ROOT` はビルド時に workspace ルートへ設定し、Cargo パッケージの
移動によって source-directory や provenance のパスが変わらないようにする。

## 互換 API と検証

ルートの `src/lib.rs` は `remacs::lisp`、`remacs::editor`、`remacs::term` などの
従来の API を再公開する。新しいコードは所有クレートへ依存する。
`remacs::term` は移行互換用であり、共通機能の実装は `remacs_runtime::frontend` に置く。

`tools/check_architecture.py` で依存の逆流とデバイス依存の配置を検査する。
変更時は workspace 全 target のコンパイル、各パッケージ単独ビルドに加え、
共通ループ・端末変換・GUI 入力・CLI 起動・埋め込みライブラリ・dump のテストを実行する。
`tests/` の統合テストは既存 API と実行ファイルの回帰を引き続き検証する。
