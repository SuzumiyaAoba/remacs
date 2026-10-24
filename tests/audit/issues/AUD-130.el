;;; -*- lexical-binding: t; -*-

;; 'japanese:character-conversion'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (japanese-hiragana 12459) (japanese-katakana 12363) (japanese-katakana 12364) (japanese-hiragana 12460) (japanese-zenkaku 97) (japanese-hankaku 12354) (japanese-hankaku 65345 t)) t))
           (error (list 'error err))))
  (terpri))

;; 'japanese:string-conversion'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (japanese-katakana "かな") (japanese-hiragana "カナ") (japanese-zenkaku "ab12") (japanese-hankaku "ＡＢ１２" t)) t))
           (error (list 'error err))))
  (terpri))

;; 'japanese:conversion-properties'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(list (get-char-code-property 12459 'hiragana) (get-char-code-property 12363 'katakana) (get-char-code-property 97 'jisx0208) (get-char-code-property 65345 'ascii)) t))
           (error (list 'error err))))
  (terpri))

;; 'japanese:explicit-reload'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (load "japan-util" nil t) (list (japanese-hiragana 12459) (japanese-katakana 12363) (japanese-zenkaku 97) (japanese-hankaku 65345 t))) t))
           (error (list 'error err))))
  (terpri))
