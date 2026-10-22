;;; -*- lexical-binding: t; -*-
;; Issue #291: autoload conversion functions together with their data.
;; Initial feature/property/cell checks precede the first conversion call.

(qa
(featurep 'japan-util)
'nil
)

(qa
(list (get-char-code-property 12459 'hiragana) (get-char-code-property 12363 'katakana) (get-char-code-property 97 'jisx0208) (get-char-code-property 65345 'ascii))
'(nil nil nil nil)
)

(qa
(mapcar (lambda (name) (autoloadp (symbol-function name))) '(japanese-hiragana japanese-katakana japanese-hankaku japanese-zenkaku japanese-hiragana-region japanese-katakana-region japanese-hankaku-region japanese-zenkaku-region setup-japanese-environment-internal read-hiragana-string))
'(t t t t t t t t t t)
)

(qa
(mapcar #'boundp '(japanese-kana-table japanese-symbol-table japanese-alpha-numeric-table))
'(nil nil nil)
)

(qa
(mapcar #'fboundp '(japanese-string-conversion japanese-replace-region))
'(nil nil)
)

(qa
(mapcar #'japanese-hiragana '(12459 12460 12497 12532 12533 12534 65393 65438))
'(12363 12364 12401 "う゛" 12533 12534 12354 65438)
)

(qa
(list (featurep 'japan-util) (boundp 'japanese-kana-table) (fboundp 'japanese-string-conversion) (get-char-code-property ?カ 'hiragana))
'(t t t 12363)
)

(qa
(mapcar #'japanese-katakana '(12354 12363 12364 12401 12436 12445 12446))
'(12450 12459 12460 12497 12436 12541 12542)
)

(qa
(mapcar (lambda (c) (japanese-katakana c t)) '(12354 12364 12401 12436 12445))
'(65393 "ｶﾞ" "ﾊﾟ" 12436 12445)
)

(qa
(mapcar #'japanese-hankaku '(12354 12364 12401 12459 12460 12532 12290 65345))
'(65393 "ｶﾞ" "ﾊﾟ" 65398 "ｶﾞ" "ｳﾞ" 65377 97)
)

(qa
(mapcar (lambda (c) (japanese-hankaku c t)) '(12354 12364 12460 65345 65281 12288))
'(12354 12364 12460 97 33 32)
)

(qa
(mapcar #'japanese-zenkaku '(97 65 48 32 126 65393 65438 65439))
'(65345 65313 65296 12288 12316 12450 12443 12444)
)

(qa
(japanese-katakana "かな がぱゔ う゛")
'"カナ ガパゔ ヴ"
)

(qa
(japanese-hiragana "カナ ガパヴ ヵヶ")
'"かな がぱう゛ ヵヶ"
)

(qa
(japanese-katakana "かな がぱ" t)
'"ｶﾅ ｶﾞﾊﾟ"
)

(qa
(japanese-hiragana "ｶﾅ ｶﾞﾊﾟｳﾞ")
'"かな がぱう゛"
)

(qa
(japanese-hankaku "かな カナ ガパヴ ＡＢ１２。")
'"ｶﾅ ｶﾅ ｶﾞﾊﾟｳﾞ AB12｡"
)

(qa
(japanese-hankaku "かな カナ ガパヴ ＡＢ１２。" t)
'"かな カナ ガパヴ AB12。"
)

(qa
(japanese-zenkaku "ab12 ｶﾅ ｶﾞﾊﾟｳﾞ")
'"ａｂ１２　カナ　ガパヴ"
)

(qa
(apply #'string (mapcar #'japanese-zenkaku (number-sequence 32 126)))
'"　！″＃＄％＆′（）＊＋，−．／０１２３４５６７８９：；＜＝＞？＠ＡＢＣＤＥＦＧＨＩＪＫＬＭＮＯＰＱＲＳＴＵＶＷＸＹＺ［￥］＾＿‘ａｂｃｄｅｆｇｈｉｊｋｌｍｎｏｐｑｒｓｔｕｖｗｘｙｚ｛｜｝〜"
)

(qa
(mapcar #'japanese-hiragana (number-sequence #xff61 #xff9f))
'(65377 65378 65379 65380 65381 12434 12353 12355 12357 12359 12361 12419 12421 12423 12387 65392 12354 12356 12358 12360 12362 12363 12365 12367 12369 12371 12373 12375 12377 12379 12381 12383 12385 12388 12390 12392 12394 12395 12396 12397 12398 12399 12402 12405 12408 12411 12414 12415 12416 12417 12418 12420 12422 12424 12425 12426 12427 12428 12429 12431 12435 65438 65439)
)

(qa
(list (get-char-code-property #x3046 'kana-composition) (get-char-code-property #xff76 'kana-composition) (get-char-code-property #xff8a 'kana-composition))
'(((12443 . 12532)) ((65438 . 12460)) ((65439 . 12497) (65438 . 12496)))
)

(qa
(let ((original (current-buffer)) (value (copy-sequence "かな")) (position (point)))
 (list (japanese-katakana value) value (eq original (current-buffer)) (= position (point))))
'("カナ" "かな" t t)
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-katakana-region (point-min) (point-max)) (buffer-string))
'"ab12 カナ カナ ｶﾞﾊﾟ ＡＢ１２。"
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-katakana-region (point-min) (point-max) t) (buffer-string))
'"ab12 ｶﾅ ｶﾅ ｶﾞﾊﾟ ＡＢ１２。"
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-hiragana-region (point-min) (point-max)) (buffer-string))
'"ab12 かな かな がぱ ＡＢ１２。"
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-hankaku-region (point-min) (point-max)) (buffer-string))
'"ab12 ｶﾅ ｶﾅ ｶﾞﾊﾟ AB12｡"
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-hankaku-region (point-min) (point-max) t) (buffer-string))
'"ab12 かな カナ ｶﾞﾊﾟ AB12。"
)

(qa
(with-temp-buffer (insert "ab12 かな カナ ｶﾞﾊﾟ ＡＢ１２。") (japanese-zenkaku-region (point-min) (point-max)) (buffer-string))
'"ａｂ１２　かな　カナ　ガパ　ＡＢ１２。"
)

(qa
(let ((before (list (japanese-hiragana ?カ) (japanese-katakana ?か) (japanese-zenkaku ?a) (japanese-hankaku ?ａ t)))) (require 'japan-util) (equal before (list (japanese-hiragana ?カ) (japanese-katakana ?か) (japanese-zenkaku ?a) (japanese-hankaku ?ａ t))))
't
)

(qa
(let ((before (list (japanese-hiragana ?カ) (japanese-katakana ?か) (japanese-zenkaku ?a) (japanese-hankaku ?ａ t)))) (load "japan-util" nil t) (list before (list (japanese-hiragana ?カ) (japanese-katakana ?か) (japanese-zenkaku ?a) (japanese-hankaku ?ａ t))))
'((12363 12459 65345 97) (12363 12459 65345 97))
)

(princ (qa--done))
