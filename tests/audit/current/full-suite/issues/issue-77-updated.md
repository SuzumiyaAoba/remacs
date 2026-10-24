## Summary

`format-time-string` diverges from GNU in zone naming, time-spec arity, and flag/width modifier handling.

## Reproduction

```elisp
(format-time-string "%H %Z" '(0 0) 0)
;; GNU:    "00 UTC"
;; remacs: "00 +00"   ← ZONE=0 not named "UTC"

(format-time-string "%Y" '(1 2 3 4 5))
;; GNU:    "1970"     ← extra spec elements ignored
;; remacs: error "Invalid time specification"

(format-time-string "%5Y" '(0 0))
;; GNU:    "01970"    ← field width
;; remacs: "5Y"       ← '%' dropped, '5' treated literally

(format-time-string "%03d" '(0 0))
;; GNU:    "001"
;; remacs: "3d"

(format-time-string "%-" '(0 0))
;; GNU:    "%-"       ← dangling flag char preserved
;; remacs: "%"
```

GNU's `Fformat_time_string` implements the full strftime-style modifier grammar (`_-0^#` flags, field widths, `:`/`::`/`:::` for %z) and only inspects `(HI LO . PS)` of the time spec.

## 全領域テストでの再確認（2026-10-18）

HEAD `29e7c960edec18a99107741c351d95444f8f6b18` の最新ビルドでも、フィールド幅と一部の複合修飾子が再現しました。

```elisp
(list (format-time-string "%5Y" 0 t)
      (format-time-string "%03d" 0 t)
      (format-time-string "%_5Y" 0 t)
      (format-time-string "%05Y" 0 t))
;; GNU Emacs 31.1: ("01970" "001" " 1970" "01970")
;; remacs:         ("5Y" "3d" "5Y" "5Y")

(list (format-time-string "%-_5Y" 0 t)
      (format-time-string "%_-5Y" 0 t))
;; GNU:    (" 1970" "1970")
;; remacs: ("_5Y" "-Y")
```

`qa_time_zero_zone` の12チェックの失敗は CLI でも再現し、起動時の noninteractive 設定や出力キャプチャの有無には依存しません。`%^B` は両方 `"JANUARY"` となり、#160 の uppercase 修正は機能しています。本 Issue の幅指定の問題は未解消のため再オープンします。

原因候補は `src/lisp/builtins/misc.rs` の `strftime_spec`: macOS の libc に GNU 固有の幅指定が渡され、そのまま文字として返る経路です。
