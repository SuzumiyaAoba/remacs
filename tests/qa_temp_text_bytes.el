;;; qa_temp_text_bytes.el --- Encode temporary initial text -*- lexical-binding: t; -*-

(defun qa327-temp-bytes (internal text coding &optional local-coding)
  (with-temp-buffer
    (when local-coding (setq-local buffer-file-coding-system local-coding))
    (let* ((parent (make-temp-file "qa327-parent-" t))
           (coding-system-for-write coding)
           (prefix (expand-file-name "text-" parent))
           (file nil))
      (unwind-protect
          (progn
            (setq file (if internal
                           (make-temp-file-internal prefix nil "" text)
                         (make-temp-file prefix nil nil text)))
            (with-temp-buffer
              (set-buffer-multibyte nil)
              (insert-file-contents-literally file)
              (string-to-list (buffer-string))))
        (delete-directory parent t)))))

;; unibyte raw bytes internal=nil
(qa (qa327-temp-bytes nil (unibyte-string 0 128 255) nil)
    '(0 128 255))

;; ASCII internal=nil
(qa (qa327-temp-bytes nil "abc" nil)
    '(97 98 99))

;; Unicode internal=nil
(qa (qa327-temp-bytes nil "é日" nil)
    '(195 169 230 151 165))

;; explicit Latin-1 internal=nil
(qa (qa327-temp-bytes nil "é" 'iso-latin-1)
    '(233))

;; DOS newlines internal=nil
(qa (qa327-temp-bytes nil "a\nb" 'utf-8-dos)
    '(97 13 10 98))

;; raw bytes and DOS newlines internal=nil
(qa (qa327-temp-bytes nil (unibyte-string 0 10 255) 'utf-8-dos)
    '(0 13 10 255))

;; multibyte raw bytes internal=nil
(qa (qa327-temp-bytes nil (string-as-multibyte (unibyte-string 128 255)) 'utf-8-unix)
    '(128 255))

;; propertized unibyte bytes internal=nil
(qa (qa327-temp-bytes nil (propertize (unibyte-string 128 255) 'audit 42) nil)
    '(128 255))

;; buffer local coding internal=nil
(qa (qa327-temp-bytes nil "é" nil 'iso-latin-1)
    '(233))

;; unibyte raw bytes internal=t
(qa (qa327-temp-bytes t (unibyte-string 0 128 255) nil)
    '(0 128 255))

;; ASCII internal=t
(qa (qa327-temp-bytes t "abc" nil)
    '(97 98 99))

;; Unicode internal=t
(qa (qa327-temp-bytes t "é日" nil)
    '(195 169 230 151 165))

;; explicit Latin-1 internal=t
(qa (qa327-temp-bytes t "é" 'iso-latin-1)
    '(233))

;; DOS newlines internal=t
(qa (qa327-temp-bytes t "a\nb" 'utf-8-dos)
    '(97 13 10 98))

;; raw bytes and DOS newlines internal=t
(qa (qa327-temp-bytes t (unibyte-string 0 10 255) 'utf-8-dos)
    '(0 13 10 255))

;; multibyte raw bytes internal=t
(qa (qa327-temp-bytes t (string-as-multibyte (unibyte-string 128 255)) 'utf-8-unix)
    '(128 255))

;; propertized unibyte bytes internal=t
(qa (qa327-temp-bytes t (propertize (unibyte-string 128 255) 'audit 42) nil)
    '(128 255))

;; buffer local coding internal=t
(qa (qa327-temp-bytes t "é" nil 'iso-latin-1)
    '(233))

(defun qa327-open-file (internal)
  (with-temp-buffer
    (let* ((parent (make-temp-file "qa327-open-" t))
           (moved (expand-file-name "moved" parent))
           (selected nil)
           (file nil))
      (fset 'qa327-rename-select
            (lambda (_start _end _coding _interactive name)
              (unless selected
                (setq selected t)
                (rename-file name moved))
              'utf-8-unix))
      (unwind-protect
          (let ((select-safe-coding-system-function 'qa327-rename-select))
            (setq file (if internal
                           (make-temp-file-internal (expand-file-name "text-" parent) nil "" "é")
                         (make-temp-file (expand-file-name "text-" parent) nil nil "é")))
            (list selected (file-exists-p file)
                  (with-temp-buffer
                    (set-buffer-multibyte nil)
                    (insert-file-contents-literally moved)
                    (string-to-list (buffer-string)))))
        (fmakunbound 'qa327-rename-select)
        (delete-directory parent t)))))

;; Coding selection may rename the already open temporary file.
(qa (qa327-open-file nil) '(t nil (195 169)))
(qa (qa327-open-file t) '(t nil (195 169)))

(princ (qa--done))
