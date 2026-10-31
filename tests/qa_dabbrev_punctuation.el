;;; -*- lexical-binding: t -*-
;; Abbreviation boundaries exclude skipped separators (#3335).
(require 'dabbrev)

(defun qa-dabbrev--extract (text &optional regexp skip)
  (with-temp-buffer
    (insert text)
    (let ((dabbrev-abbrev-char-regexp regexp)
          (dabbrev-abbrev-skip-leading-regexp skip))
      (dabbrev--reset-global-variables)
      (dabbrev--abbrev-at-point))))

(qa (qa-dabbrev--extract "foo\nfoo.") "foo")
(qa (qa-dabbrev--extract "foo?!") "foo")
(qa (qa-dabbrev--extract "foo \t\n") "foo")
(qa (qa-dabbrev--extract "foo") "foo")
(qa (qa-dabbrev--extract "foo_bar.") "foo_bar")
(qa (qa-dabbrev--extract "alpha βήτα!") "βήτα")
(qa (qa-dabbrev--extract "日本語。") "日本語")
(qa (qa-dabbrev--extract "foo." "[a-z.]") "foo.")
(qa (qa-dabbrev--extract "foo_bar." "[a-z]") "bar")
(qa (qa-dabbrev--extract "$foo." "[$a-z]" "[$]") "foo")

(qa (with-temp-buffer
      (insert (propertize "foo." 'face 'bold))
      (set-buffer-modified-p nil)
      (dabbrev--reset-global-variables)
      (string-match "\\(b\\)" "abc")
      (let ((position (point))
            (matches (match-data))
            (buffer-read-only t))
        (let ((abbrev (dabbrev--abbrev-at-point)))
          (list abbrev (text-properties-at 0 abbrev)
                (= position (point)) (equal matches (match-data))
                (buffer-modified-p)))))
    '("foo" nil t t nil))

(qa (with-temp-buffer
      (insert "prefix foo.! suffix")
      (narrow-to-region 8 13)
      (goto-char (point-max))
      (dabbrev--reset-global-variables)
      (dabbrev--abbrev-at-point))
    "foo")
(qa-err (qa-dabbrev--extract "") 'user-error)
(qa-err (qa-dabbrev--extract "... \n") 'user-error)

(defun qa-dabbrev--expand (source repeat)
  (with-temp-buffer
    (insert source)
    (let ((dabbrev-check-other-buffers nil)
          (dabbrev-check-all-buffers nil)
          (dabbrev-backward-only t)
          (last-command nil)
          (this-command 'dabbrev-expand))
      (dabbrev--reset-global-variables)
      (dabbrev-expand nil)
      (when repeat
        (setq last-command 'dabbrev-expand)
        (dabbrev-expand nil))
      (buffer-string))))

(qa (qa-dabbrev--expand "foobar\nfo." nil) "foobar\nfoobar.")
(qa (qa-dabbrev--expand "foobaz foobar\nfo?!" t) "foobaz foobar\nfoobaz?!")

(princ (qa--done))
