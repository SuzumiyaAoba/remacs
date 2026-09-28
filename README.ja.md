# remacs

GNU Emacs 31.1 互換のエディタを Rust で実装したものです。Lisp
ランタイム（reader・評価器・バイトコード VM・printer）、
バッファ/エディタコア、フロントエンドはすべてネイティブ Rust で、
実際の GNU Emacs Lisp ライブラリ（`lisp/` 以下 1,600+ ファイル、
GNU Emacs 31.1 からバイト同一に移植）をそのまま実行します。

> **補足:** 名前は同じですが、元の remacs プロジェクト（Emacs の C
> コアを Rust に移植したもの）ではありません。本プロジェクトでは
> ランタイム全体が Rust で、GNU Emacs と共有するのは Lisp
> ライブラリと外部挙動です。

## 機能

- **Emacs Lisp ランタイム** — 完全な評価器：スペシャルフォーム、
  マクロ、レキシカル＋ダイナミック binding、クロージャ、
  `catch`/`throw`、`unwind-protect`、GNU と同じ透過性セマンティクスを
  持つ `symbols-with-pos`（位置付きシンボル）、`defvar`/`defcustom`、
  advice、`cl-generic`、ダンプ済みライブラリの feature モデル。
- **バイトコード実行** — 本物の GNU `#[...]` 実行器
  （`src/lisp/bytecode.rs`）：`.elc` のロードと実行に対応し、
  `byte-switch` のジャンプテーブル、ハンドラフレーム、
  インタプリタの `catch_tags` 契約も実装。`byte-compile-file` が
  実際のバイトコードを生成します。
- **ネイティブコンパイル** — libgccjit ではなく生成 C パイプライン：
  `.el` → C → システム `cc` → `.eln` 共有オブジェクト → `dlopen`。
  `cc` が使える環境では `(native-comp-available-p)` が `t`。
  `-f` 経由で `batch-native-compile` も利用可能。
- **フロントエンド**
  - crossterm による端末 UI（`-nw` / TTY がデフォルト）。
  - [gpui](https://gpui.rs) による GPU 描画 GUI — 評価器は専用の
    ロジックスレッドに置き、ウィンドウは `render_grid` のスナップショットを
    latest-wins チャネルで描画するため、Lisp 実行中も UI が
    ブロックしません。
  - macOS NeXTstep レイヤ（`src/lisp/builtins/nsgui.rs`）：dlopen した
    `libobjc.A.dylib` + `objc_msgSend` 経由で本物の AppKit /
    Foundation を呼び出します — `ns-list-colors`、`x-create-frame`、
    `x-file-dialog`、`ns-badge`、`ns-begin-drag`、サービス等。
- **ライブラリカバレッジ** — GNU Emacs 31.1 の Lisp ツリー全体が
  `lisp/` 以下にフラットに置かれ、`EMBEDDED_LISP`
  （`src/lisp/load.rs`）で埋め込まれています。`require`/`load` は
  これに対して解決します。起動は `src/lisp/prelude.el` が駆動します。
- **データ型とプリミティブ** — バッファ（gap buffer）、マーカー、
  window/frame、ハッシュテーブル、char-table、bool-vector、
  record、正規表現、CJK テーブル含む charset、プロセス、タイマー、
  sqlite3、JSON、XML/HTML（html5ever）、tree-sitter 文法。
- **外部ライブラリ** — 実行時に dlopen する任意ライブラリ：
  GnuTLS（暗号 + TLS セッション）、Little CMS 2、D-Bus
  （同期/非同期呼出・シグナル・サービス登録）、kqueue
  file-notify。
- **Tree-sitter** — `treesit-*` プリミティブ、parser/node/query
  record、述語、notifier、included range。文法は GNU と同様に
  `~/.emacs.d/tree-sitter/` 等からロードします。

## ビルド

```sh
cargo build            # デバッグ
cargo build --release  # 最適化
```

Rust 2024 edition。crates 以外のビルド時依存はありません。
任意ライブラリ（`libgnutls`、`liblcms2`、`libdbus-1`）は実行時に
dlopen されるため、なくても remacs は動作します（対応する feature が
提供されないだけです）。ネイティブコンパイルには動作する `cc` が
必要です。

## 実行

```sh
remacs                     # GUI（gpui）/ TUI（プラットフォーム依存）
remacs -nw                 # 端末 UI
remacs FILE...             # ファイルを開く
remacs --batch --eval '(message "hi %s" 42)'
remacs -Q --batch -l myfile.el
remacs --script script.el
remacs --batch -f batch-native-compile lisp/foo.el
remacs --version
```

バッチモードは GNU と同様に `--funcall/-f`、`--load/-l`、
`--eval`/`--execute` を処理し、`command-line-args-left` を消費します。

## 開発

診断用環境変数（すべて env ゲート付き。未設定なら無効）：

| 変数 | 効果 |
| --- | --- |
| `PRELUDE_MAX` | prelude 評価を打ち切る（init の二分探索用） |
| `--ieval` | トップレベルフォームを対話的に二分探索 |
| `WHILE_WATCH` | eval レベルの `while` ループをトレース |
| `REMACS_TRACE_BC` | バイトコードの PC/opcode/スタックをトレース |
| `REMACS_BT_ERR` | `signal` 時に Lisp スタックをダンプ（内容フィルタ指定可） |
| `DBG_FUNCALL` | `funcall`/呼出しディスパッチをトレース |

`lisp/*.el` を追加・削除した後は `EMBEDDED_LISP` エントリを再生成：

```sh
python3 tools/gen_embedded.py
```

### テスト

```sh
cargo test --test lisp_core     # 評価器/reader/print/equality コア
cargo test --test lisp_libs     # Lisp ライブラリプローブ
cargo test --test nativecomp    # .eln パイプライン（cc 必須）
cargo test --test nsgui         # macOS 限定の NS プリミティブ
cargo test --test extlib        # gnutls/lcms2/dbus/filenotify（低速）
cargo test --test compat        # GNU parity のコマンド検証
```

注意：ほとんどの `ev()` 呼出しは prelude 全体を起動するため各 ~30 秒
かかり、スイートは重いです。テスト名を指定して個別実行を推奨します。

移植済みファイルと GNU ソースのバイト比較：

```sh
GNU=/nix/store/...-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/emacs-lisp/byte-opt.el.gz | cmp - lisp/byte-opt.el
```

## 既知の制限

- `xwidget-internal` は未実装。
- GNU の内部 emacs-mule エンコーディングだった 13 ファイルは UTF-8
  へ変換済み — GNU が私用面文字を置いていた箇所は `U+FFFD`。
- ネイティブコンパイル：1 ユニットあたり最大 256 関数
  （トランポリンプール）。`comp.el` の LIMPLE/最適化パスは未実行。
  `comp-libgccjit-version` → nil。
- 起動時に無害な `void-variable`/`bool-vector-p` のシグナルが
  バッチでいくつか出力される。`byte-opt` が "lambda used as
  function name" 警告を出す。
- プラットフォーム限定ファイル（`*-win`、android）は GNU と同様に
  macOS ではロードに失敗します。

詳細な移植ノート（レイアウト、EMBEDDED_LISP の規則、
NS/ネイティブコンパイルのアーキテクチャ、検証手順）は
`AGENTS.md` を参照してください。

## ライセンス

GNU General Public License v3（`LICENSE` 参照）。`lisp/` と `etc/`
以下の Lisp ライブラリファイルは GNU Emacs 31.1 由来で、それぞれの
著作権表示を保持しています。
