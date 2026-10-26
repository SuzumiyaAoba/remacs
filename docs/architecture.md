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

`lisp/eval.rs` は評価・関数呼び出しのディスパッチと、既存公開パスの再公開を保持する。
補助処理は `lisp/eval/` の非公開モジュールに分け、既存の `eval::*` 公開パスを再公開で維持する。

- `lexenv.rs`: 字句環境の永続的なリスト構造、共有束縛セル、検索・代入・動的宣言。
- `bindings.rs`: 動的束縛スタック、バッファローカル／デフォルト値の保存と復元、巻き戻し。
- `conditions.rs`: signal の生成、診断、条件名の照合。
- `control.rs`: Lisp 評価深度、signal の配送、デバッガ用の余裕、quit の巻き戻し。
- `symbols.rs`: シンボル名・位置、変数セルと alias、プロパティ、watcher の通知。
- `functions.rs`: 関数セルと隠れた定義の解決、組み込み関数の同一性、再定義履歴。
- `source.rs`: 文字列／バッファ評価の開始と読み取り位置。
- `macros.rs`: マクロ呼び出しの文脈、展開ループ、引数エラーの変換。
- `objects.rs`: overlay と文字列の同一性に付随する表現・プロパティ情報。
- `buffers.rs`: バッファ作成・選択、標準 case/category table の管理。
- `plist.rs`: シンボルプロパティで使う property list の検索と変更。
- `bootstrap.rs`: ストレージ生成、組み込み機能登録、Lisp イメージ構築、セッション開始を順に実行。
  `bootstrap/` に初期値、非公開シンボル一覧、ドキュメント探索、履歴解析、シンボル状態、
  コンパイル時だけ必要なライブラリの巻き戻しを分離する。
  `defaults/` は条件・変数ポリシーの宣言と初期値・環境値の生成を所有する。
  初期値は全て構築してから代入し、シンボルの intern と初期化の順序を保つ。
  `rollback/` は履歴／ソースから復元計画を収集してから、セル・プロパティ・履歴・
  registry・autoload の順で適用する。kept library の定義が復元対象より優先する。
- `lambda.rs`: クロージャ生成、引数束縛、解釈／バイトコード本体の呼び出し。
- `advice.rs`: advice の合成・解除と元の関数の取得。
- `interactive.rs`: コマンド実行と対話引数の生成。`interactive/parser.rs` は文字列の走査のみを行い、
  実行側は各行の prompt を整形してから制御文字を検証・処理する。仕様表は `interactive/specs.rs`。
- `minibuffer.rs`: ミニバッファの読み取り、履歴、フロントエンド／バッチ入力。
- `state.rs`: 実行状態のリセット、評価の出自、ラムダ束縛、ミニバッファ呼び出し元、
  point／narrowing の保存と復元。`state/interpreter.rs` は `Interp` の格納フィールドを所有し、
  非公開フィールドの可視範囲は従来通り `eval` 内に限定する。
- `output.rs`: 出力先の検証と文字の出力、エコー表示、`*Messages*` の履歴管理。

初期化では prelude 前の plist、prelude 後のシンボル状態、巻き戻し後の gv 定義を
それぞれの時点で保存する。遅延 custom 初期化後に gv 定義を再適用する順序を変えない。
公開フィールドの配置は既存 API のため維持し、関連する保存値は名前付きの型でまとめる。
ラムダ引数の束縛が失敗した場合も呼び出し元の字句環境を戻し、その後に動的束縛を解除する。


最適化時も、字句環境のセル共有と最初の一致で検索を止める規則、watcher の通知順、
巻き戻し中に発生した signal の配送順を維持する。状態を複製した別の実行経路は作らない。
束縛と条件の変更には `qa_eval`、`qa_eval_lexenv`、`qa_closure_constructor`、
`qa_variable_watchers`、`qa_local_dynamic_unwind`、`qa_signal_*`、`qa_plists` を使い、
通常起動と `REMACS_NO_ELC=1` のソース起動の両方で確認する。

### Lisp reader の内部構成

`lisp/reader.rs` は入力カーソル、layout、リストとシンボルの読み取りを所有する。
入力元は固定文字列か関数ストリームかを区別し、EOF 状態は関数ストリームだけが保持する。

- `reader/quoted.rs`: 文字列・文字リテラル、エスケープと修飾ビット。
- `reader/numbers.rs`: 完全なトークンに対する整数・浮動小数点の判定。`reader::parse_number` を維持する。
- `reader/dispatch.rs`: `#` 構文、record／hash-table リテラルの構築。
- `reader/circles.rs`: 番号付きオブジェクトの placeholder を循環グラフ上で補修する。

関数入力は必要な位置までだけ取得し、先読みの unread は逆順に返す。
エラーでも消費済みの位置を維持し、unread が成功して文字を切り戻したときだけ EOF を解除する。

### ローダー・printer・bytecode

`lisp/load.rs` は探索結果とAOTポリシーを選び、各処理へ渡す入口を保持する。
既存の `load::*` 公開パスは再公開で維持する。

- `load/command.rs`: コマンドラインによる load-path の cons セル単位の差し込み。
- `load/locate.rs`: load-path 検索、組み込みキー、shipped source 判定とソースの公開。
- `load/file.rs`: 生バイトのデコード、gzip、512バイトのコンパイラヘッダ検査。
- `load/source.rs`: 名前付きの `LoadRequest` と `LoadCaller` による評価・状態復元。
- `load/eager.rs`: 入力位置を更新しながら行うトップレベル評価と eager macro expansion。
- `load/history.rs`: load-history の構築・マージ、定義／依存関係の記録。
- `load/lexical.rs`: lexical cookie と既定の字句束縛コールバック。
- `load/compat.rs`: GNU subr-x の再ロード時に既存定義を保護する処理。
- `load/policy.rs`: AOT を選べるライブラリの宣言データ。

読み取りエラーは load の束縛や入力バッファを除く前に通知する。字句環境と excursion を戻し、
動的束縛を解除してから直接入力ストリームを戻す。Lisp を呼ぶ後始末を Drop に移さない。

`lisp/print.rs` は値の種類ごとの出力を保持する。`print/render.rs` の `PrintMode` が
readable/display の共通経路を選び、`print/lists.rs` が共有のリスト走査と循環検出を行う。
`print/circle.rs` は同一性・番号表、`print/objects.rs` は record／closure／char-table の構文、
`print/numbers.rs` は数値表記を所有する。コールバックを呼ぶ前に印字用TLSを復元する。
純粋な印字中の Rust unwind でもそのTLSを戻すが、Drop から Lisp は呼ばない。

`lisp/bytecode.rs` は引数記述子・キャッシュ・実行境界・ハンドラの検索を保持する。
実行ごとの `Frame` が命令位置、値スタック、巻き戻し記録、ハンドラを所有し、
`bytecode/opcodes.rs` に渡す。opcode 内の検査と副作用の順序、signal を観測させてから
巻き戻す順序、ハンドラ→巻き戻し記録→値スタックの解放順を維持する。

### 正規表現と組み込み関数の所有範囲

`lisp/regexp.rs` は公開型、syntax/category の接続と互換の公開パスを保持する。
`regexp/parser.rs` は mutable な解析位置と開いた捕捉を所有し、完成したASTと捕捉数だけを返す。
`regexp/codegen.rs` はそのASTを順序付きの命令列へ変換し、`regexp/matching.rs` は
不変の `MatchContext` と再帰ごとの位置・捕捉を使って照合する。分岐順・後方探索の上限・
空反復の進捗検査・POSIX最長一致を別々の規則として維持する。

`builtins/misc.rs` は単一の `SUBRS` 登録表と互換用の再公開、共通定数／型だけを中心に保持する。
実装は `misc/` の関数情報、位置付きシンボル、時刻、暗号／base64、ファイル、利用者情報、
coding、category／char-table、keymap、event、thread／同期、record／bool-vector、
diagnostic／DOC、face／font、display／window、image、buffer、platform互換の各領域に置く。
既存の `misc::*` 呼び出し先は再公開で維持する。登録表の順序とプリミティブの名前・arity・
定義の対応を変えず、領域間で必要な共通ヘルパーは明示的に import する。

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
