;;; qa_blank_line_characters.el --- Blank-line classification -*- lexical-binding: nil -*-
;; Issue #269: only spaces and tabs make a line blank.  In particular,
;; Rust's Unicode whitespace classification must not erase NBSP or CR.
;; The GNU simple.el implementation installed for #270 also fixes this.
;; Run with qa_framework.el; expectations verified with GNU Emacs 31.1.

(defun qa269-delete (text position)
  (with-temp-buffer
    (insert text)
    (goto-char position)
    (let ((result (delete-blank-lines)))
      (list result (buffer-string) (point)))))

;; The exact reported cases, including the ordinary space/TAB control.
(qa (qa269-delete "head\n \nbody\n" 6) '(nil "head\n \nbody\n" 6))
(qa (qa269-delete "head\n\r\nbody\n" 6) '(nil "head\n\r\nbody\n" 6))
(qa (qa269-delete "head\n \t\nbody\n" 6) '(nil "head\nbody\n" 6))

;; Every other Unicode White_Space character is nonblank, including
;; vertical tab, form feed, CR, NEL, NBSP, and line/paragraph separators.
;; Zero-width space, BOM, and Mongolian vowel separator are controls for
;; characters that are not in the current Unicode White_Space property.
(dolist (code '(11 12 13 133 160 5760
                8192 8193 8194 8195 8196 8197 8198 8199 8200 8201 8202
                8232 8233 8239 8287 12288 8203 65279 6158))
  (let* ((character (string code))
         (isolated (concat "head\n" character "\nbody\n"))
         (padded (concat "head\n \t" character "\t \nbody\n")))
    (qa (qa269-delete isolated 6) (list nil isolated 6))
    ;; Classification is independent of where point is on the line.
    (dolist (position '(6 8 11))
      (qa (qa269-delete padded position) (list nil padded position)))
    ;; From a nonblank line, delete following blank lines only.
    (qa (qa269-delete (concat "head\n\n" character "\n\nbody\n") 7)
        (list nil (concat "head\n\n" character "\nbody\n") 7))
    ;; From the preceding blank line, stop before the nonblank character.
    (qa (qa269-delete (concat "head\n\n" character "\nbody\n") 6)
        (list nil isolated 6))
    ;; A final nonblank line stays intact, with or without its newline.
    (dolist (suffix '("" "\n"))
      (let ((text (concat "head\n" character suffix)))
        (qa (qa269-delete text 6) (list nil text 6))))))

;; Spaces and tabs remain blank; isolated lines disappear at all edges.
(dolist (blank '("" " " "\t" " \t \t"))
  (qa (qa269-delete (concat "head\n" blank "\nbody\n") 6)
      '(nil "head\nbody\n" 6))
  (qa (qa269-delete (concat blank "\nbody\n") 1)
      '(nil "body\n" 1))
  (qa (qa269-delete (concat "head\n" blank "\n") 6)
      '(nil "head\n" 6)))

;; Classification is literal, not controlled by the syntax table.
(dolist (code '(13 160))
  (qa (with-temp-buffer
        (set-syntax-table (copy-syntax-table))
        (modify-syntax-entry code " ")
        (insert "head\n" (string code) "\nbody\n")
        (goto-char 6) (delete-blank-lines)
        (string-to-list (buffer-string)))
      (append '(104 101 97 100 10) (list code) '(10 98 111 100 121 10))))

;; Keep the #270 restriction boundary even when the visible line is only
;; the nonblank character.  Hidden read-only spaces cannot be deleted.
(dolist (code '(13 160 8195))
  (qa (with-temp-buffer
        (insert "head\n " (string code) " \nbody\n")
        (put-text-property 1 7 'read-only 'hidden)
        (put-text-property 8 (point-max) 'read-only 'hidden)
        (narrow-to-region 7 8) (goto-char 7)
        (let ((result (delete-blank-lines)))
          (list result (point-min) (point-max) (point)
                (string-to-list (buffer-string)))))
      (list nil 7 8 7 (list code))))

;; Raw bytes in unibyte buffers must not become deletable whitespace.
(dolist (code '(11 12 13 133 160 255))
  (qa (with-temp-buffer
        (set-buffer-multibyte nil)
        (insert (unibyte-string 65 10 code 10 66))
        (goto-char 3) (delete-blank-lines)
        (list (string-to-list (buffer-string))
              (multibyte-string-p (buffer-string)) (point)))
      (list (list 65 10 code 10 66) nil 3)))

(princ (qa--done))
(terpri)
