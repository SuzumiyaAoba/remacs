;;; qa_ps_nb_pages.el --- Regression tests for ps-print page estimates -*- lexical-binding:t -*-

(require 'ps-print)

(qa
 (let ((saved-char-width (get 'ps-font-for-text 'avg-char-width))
       (saved-space-width (get 'ps-font-for-text 'space-width))
       result)
   (unwind-protect
       (setq result
             (with-temp-buffer
               (put 'ps-font-for-text 'avg-char-width 10.0)
               (put 'ps-font-for-text 'space-width 5.0)
               (insert (make-string 1000 ?x) "\nshort\n")
               (let ((ps-print-width 100.0))
                 (let* ((from (point-min))
                        (to (point-max))
                        (widths (ps-nb-pages-line-widths from to))
                        (short-start (progn (goto-char (point-min))
                                            (forward-line 1)
                                            (point)))
                        (short-end (progn (goto-char short-start)
                                          (end-of-line)
                                          (point)))
                        (short-widths
                         (ps-nb-pages-line-widths short-start short-end))
                        (font-size
                         (ps-nb-pages-font-size-for-pages
                          10 2 widths 4 14 10 10 0 20)))
                   (list (length widths)
                         (ps-nb-pages-folded-lines widths 10 10)
                         (ps-nb-pages-folded-lines widths 4 10)
                         (ps-nb-pages-count-at-font-size
                          2 widths 10 10 10 0 20)
                         (ps-nb-pages-count-at-font-size
                          2 widths 4 10 10 0 20)
                         (ps-nb-pages-folded-lines short-widths 14 10)
                         (and (<= 4 font-size 14)
                              (<= (ps-nb-pages-count-at-font-size
                                   2 widths font-size 10 10 0 20)
                                  10)))))))
     (put 'ps-font-for-text 'avg-char-width saved-char-width)
     (put 'ps-font-for-text 'space-width saved-space-width))
   result)
 '(2 99 39 51 9 0 t))

(defun qa-ps-nb-pages-test-dimensions ()
  (setq ps-print-width 100.0)
  (setq ps-print-height 20.0))

(defun qa-ps-nb-pages-test-average-width (_font)
  10.0)

(defun qa-ps-nb-pages-test-space-width (_font)
  5.0)

(defun qa-ps-nb-pages-test-line-height (_font)
  10.0)

(defun qa-ps-nb-pages-test-setup ()
  "setup")

(defun qa-ps-nb-pages-test-display-buffer (buffer &rest _args)
  buffer)

(qa
 (let* ((ps-font-size-internal 10)
        (ps-header-font-size-internal 10)
        (ps-footer-font-size-internal 10)
        (ps-header-title-font-size-internal 10)
        (ps-line-spacing-internal 0)
        (ps-print-width nil)
        (ps-print-height nil)
        (old-page-dimensions (symbol-function 'ps-get-page-dimensions))
        (old-average-width (symbol-function 'ps-avg-char-width))
        (old-space-width (symbol-function 'ps-space-width))
        (old-line-height (symbol-function 'ps-line-height))
        (old-setup (symbol-function 'ps-setup))
        (old-display-buffer (symbol-function 'display-buffer))
        outputs)
   (unwind-protect
       (progn
         (fset 'ps-get-page-dimensions #'qa-ps-nb-pages-test-dimensions)
         (fset 'ps-avg-char-width #'qa-ps-nb-pages-test-average-width)
         (fset 'ps-space-width #'qa-ps-nb-pages-test-space-width)
         (fset 'ps-line-height #'qa-ps-nb-pages-test-line-height)
         (fset 'ps-setup #'qa-ps-nb-pages-test-setup)
         (fset 'display-buffer #'qa-ps-nb-pages-test-display-buffer)
         (setq outputs
               (with-temp-buffer
                 (insert (make-string 1000 ?x) "\n")
                 (let ((from (point-min))
                       (to (point-max))
                       (source (current-buffer))
                       buffer-output
                       region-output)
                   (ps-nb-pages-buffer 1 from to)
                   (setq buffer-output
                         (with-current-buffer "*Nb-Pages*" (buffer-string)))
                   (kill-buffer "*Nb-Pages*")
                   (set-buffer source)
                   (ps-nb-pages-region 1 from to)
                   (setq region-output
                         (with-current-buffer "*Nb-Pages*" (buffer-string)))
                   (list buffer-output region-output))))
     (fset 'ps-get-page-dimensions old-page-dimensions)
     (fset 'ps-avg-char-width old-average-width)
     (fset 'ps-space-width old-space-width)
     (fset 'ps-line-height old-line-height)
     (fset 'ps-setup old-setup)
     (fset 'display-buffer old-display-buffer)
     (when (get-buffer "*Nb-Pages*")
       (kill-buffer "*Nb-Pages*")))
   (and (string-match-p
         "There are 1 logical lines (long-line folding is estimated)"
         (car outputs))
        (string-match-p "\n[ \t]+8   " (car outputs))
        (string-match-p
         "There are 1 logical lines (long-line folding is estimated)"
         (cadr outputs))
        (string-match-p "\n[ \t]+8   " (cadr outputs))
        t)))
 t)

(princ (qa--done))
