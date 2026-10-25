;;; -*- lexical-binding: nil; -*-
;; Word motion and counts split Japanese text by script (#548).
(qa
 (with-temp-buffer
   (insert "の文章")
   (goto-char (point-min))
   (forward-word-strictly 1)
   (list (point) (count-words-region (point-min) (point-max))))
 '(2 2))
(qa
 (with-temp-buffer
   (insert "の文章")
   (goto-char (point-min))
   (forward-word 1)
   (point))
 2)
(qa
 (with-temp-buffer
   (insert "の文章")
   (goto-char (point-max))
   (backward-word-strictly 1)
   (point))
 2)
(qa
 (with-temp-buffer
   (insert "の文章")
   (goto-char (point-min))
   (thing-at-point 'word))
 "の")
(qa (with-temp-buffer
      (insert "ひらがなカタカナ漢字")
      (count-words-region (point-min) (point-max)))
    3)
(qa (with-temp-buffer
      (insert "あAい")
      (count-words-region (point-min) (point-max)))
    3)
(princ (qa--done))
