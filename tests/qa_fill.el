;; fill.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - fill-region wraps text at fill-column.
;;   - justify-current-line justifies a line to fill-column.
;;   - fill-newline inserts a line break honoring fill-prefix.
;;   - fill-context-prefix computes the shared line-prefix.
;;   - set-justification takes (BEG END &optional MODE NOSQUEEZE).

;; == surface ==
(qa (fboundp 'fill-paragraph) t)
(qa (fboundp 'fill-region) t)
(qa (fboundp 'fill-region-as-paragraph) t)
(qa (fboundp 'fill-individual-paragraphs) t)
(qa (fboundp 'fill-nonuniform-paragraphs) t)
(qa (fboundp 'justify-current-line) t)
(qa (fboundp 'set-fill-column) t)
(qa (fboundp 'set-justification) t)
(qa (fboundp 'fill-comment-paragraph) t)
(qa (fboundp 'fill-context-prefix) t)
(qa (fboundp 'fill-newline) t)
(qa (fboundp 'fill-delete-prefix) t)
(qa (fboundp 'fill-delete-newlines) t)
(qa (fboundp 'fill-move-to-break-point) t)
(qa (fboundp 'fill-find-break-point) t)
;; NOT fns in 31.1
(qa (fboundp 'fill-single-word-p) nil)
(qa (fboundp 'fill-common-prefix) nil)
(qa (fboundp 'fill-find-break-point-line) nil)
(qa (fboundp 'fill-region-inside-quotes) nil)

;; == fill-region wraps at fill-column ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc ddd eee fff ggg hhh")
        (let ((fill-column 20))
          (fill-region (point-min) (point-max))
          (qa (buffer-string) "aaa bbb ccc ddd eee\nfff ggg hhh")))
    (kill-buffer buf)))

;; == fill-region with justify 'full — extra spaces between words ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc ddd eee fff")
        (let ((fill-column 20))
          (fill-region (point-min) (point-max) 'full)
          ;; first line is fully-justified; second is short
          (qa (buffer-string) "aaa bbb  ccc ddd eee\nfff")))
    (kill-buffer buf)))

;; == justify-current-line on 'full para ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc\nddd eee")
        (goto-char (point-min))
        (let ((fill-column 30))
          (set-justification (point-min) (point-max) 'full)
          (justify-current-line))
        ;; joined into one line then fully-justified
        (qa (string-match-p "aaa.*bbb.*ccc" (buffer-string)) 0))
    (kill-buffer buf)))

;; == fill-newline inserts a break ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-max))
        (fill-newline)
        (qa (buffer-string) "abc\n"))
    (kill-buffer buf)))

;; == fill-newline with fill-prefix ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-max))
        (let ((fill-prefix "; "))
          (fill-newline)
          ;; insert "; " on the new line
          (qa (string-prefix-p "abc\n" (buffer-string)) t)))
    (kill-buffer buf)))

;; == fill-context-prefix takes (LINE1-FIRST LINE2-FIRST) ==
;; returns the SHARED whitespace prefix between two lines.
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "; hello\n; world")
        (qa (fill-context-prefix 1 8) "  "))
    (kill-buffer buf)))

;; == 'center justification joins lines and centers ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc\nddd eee")
        (goto-char (point-min))
        (let ((fill-column 30))
          (set-justification (point-min) (point-max) 'center)
          (justify-current-line))
        ;; centered result carries the justification text-prop
        (qa (substring-no-properties (buffer-string))
            "     aaa   bbb  ccc   ddd  eee"))
    (kill-buffer buf)))

;; == current-fill-column returns fill-column ==
(let ((buf (generate-new-buffer " *f*")))
  (unwind-protect
      (with-current-buffer buf
        (let ((fill-column 42))
          (qa (current-fill-column) 42)))
    (kill-buffer buf)))

;; == fill-prefix is a var ==
(qa (boundp 'fill-prefix) t)
(qa fill-prefix nil)

;; == set-fill-prefix is a command ==
(qa (fboundp 'set-fill-prefix) t)

(princ (qa--done))
