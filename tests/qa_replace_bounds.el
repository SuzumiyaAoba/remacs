;;; -*- lexical-binding: t; -*-
;; Issue #65: range errors report the match registers, not NEWTEXT's
;; length or its escape expansion. Set registers explicitly: startup
;; and preceding regexp operations may leave different match data.
(defvar qa65-replacement)
(dolist (qa65-replacement '("x\\,y" "a" "a much longer string" "x\\" "日\\,語"))
  (qa-err-data (with-temp-buffer
                 (set-match-data '(0 2))
                 (replace-match qa65-replacement)) 'args-out-of-range '(0 2))
  (qa-err-data (with-temp-buffer
                 (set-match-data '(0 3))
                 (replace-match qa65-replacement)) 'args-out-of-range '(0 3))
  (qa-err-data (with-temp-buffer
                 (set-match-data '(0 2))
                 (replace-match qa65-replacement t t)) 'args-out-of-range '(0 2)))

;; String matches use zero-based character indices, even when a later
;; replace-match targets the current buffer instead of that string.
(qa-err-data (with-temp-buffer
               (string-match "ab" "abc")
               (replace-match "x\\,y")) 'args-out-of-range '(0 2))
(qa-err-data (with-temp-buffer
               (string-match "日a日" "日a日")
               (replace-match "x\\,y")) 'args-out-of-range '(0 3))
(qa-err-data (progn (set-match-data '(0 4))
                    (replace-match "x\\,y" nil nil "日a日"))
             'args-out-of-range '(0 4))
(qa-err-data (with-temp-buffer
               (insert "abc") (goto-char 1) (search-forward "a")
               (narrow-to-region 2 4)
               (replace-match "x\\,y")) 'args-out-of-range '(1 2))

;; A valid empty match reaches replacement validation. LITERAL bypasses
;; escape validation; it does not bypass validation of the match range.
(qa-err (with-temp-buffer
          (set-match-data '(1 1))
          (replace-match "x\\,y")) 'error)
(qa (with-temp-buffer
      (set-match-data '(1 1))
      (replace-match "x\\,y" t t)
      (buffer-string)) "x\\,y")
(qa (progn (set-match-data '(0 2))
            (replace-match "x\\,y" t t "abc")) "x\\,yc")
(qa (with-temp-buffer
      (insert "abc")
      (set-match-data '(0 2))
      (condition-case nil (replace-match "x\\,y") (args-out-of-range nil))
      (list (buffer-string) (match-data))) '("abc" (0 2)))

(princ (qa--done)) (terpri)
