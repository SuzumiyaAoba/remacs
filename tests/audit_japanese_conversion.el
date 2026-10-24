;;; audit_japanese_conversion.el --- kana and width conversion -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "japanese:character-conversion"
 '(list (japanese-hiragana ?カ) (japanese-katakana ?か)
        (japanese-katakana ?が) (japanese-hiragana ?ガ)
        (japanese-zenkaku ?a) (japanese-hankaku ?あ)
        (japanese-hankaku ?ａ t)))

(audit-check "japanese:string-conversion"
 '(list (japanese-katakana "かな") (japanese-hiragana "カナ")
        (japanese-zenkaku "ab12") (japanese-hankaku "ＡＢ１２" t)))

(audit-check "japanese:conversion-properties"
 '(list (get-char-code-property ?カ 'hiragana)
        (get-char-code-property ?か 'katakana)
        (get-char-code-property ?a 'jisx0208)
        (get-char-code-property ?ａ 'ascii)))

(audit-check "japanese:explicit-reload"
 '(progn
    (load "japan-util" nil t)
    (list (japanese-hiragana ?カ) (japanese-katakana ?か)
          (japanese-zenkaku ?a) (japanese-hankaku ?ａ t))))

(princ "AUDIT-DONE\n")
