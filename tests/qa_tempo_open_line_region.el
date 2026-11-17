;;; qa_tempo_open_line_region.el --- Tempo o tag inserts a newline before regions -*- lexical-binding: t; -*-

(require 'tempo)

(qa
 (progn
   (set (intern "tempo-audit-open-line-template")
        (list (intern "o") "X" (intern "r")))
   (with-temp-buffer
     (insert "abc")
     (set-mark (point-min))
     (goto-char (point-max))
     (let ((transient-mark-mode t)
           (mark-active t))
       (tempo-insert-template (intern "tempo-audit-open-line-template") t)
       (buffer-string))))
 "X\nabc")

(princ (qa--done))
