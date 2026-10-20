;;; -*- lexical-binding: t; -*-
;; GNU safe_to_load_version probes: a marker alone is insufficient.
(defvar qa123-ran nil)
(defvar qa123-root (make-temp-file "qa123-header-" t))
(defvar qa123-file (expand-file-name "module.elc" qa123-root))
(defconst qa123-magic ";ELC\037\0\0\0\n")
(defconst qa123-header ";;; in Emacs version 31.1\n")
(defun qa123-write (text &optional file)
  (let ((coding-system-for-write 'binary) (file-name-handler-alist nil))
    (write-region text nil (or file qa123-file) nil 'quiet)))
(defun qa123-load (text &optional file noerror)
  (setq qa123-ran nil)
  (qa123-write text file)
  (condition-case error
      (list (load (or file qa123-file) noerror t t) qa123-ran)
    (error (list (car error) (cdr error) qa123-ran))))
(defun qa123-rejection ()
  (list 'error (list (format-message "File `%s' was not compiled in Emacs" qa123-file)) nil))
(unwind-protect
    (progn
      (qa (qa123-load "") '(t nil))
      (qa (qa123-load "(setq qa123-ran t)\n") (qa123-rejection))
      (qa (qa123-load "(setq qa123-ran t)\n" nil t) (qa123-rejection))
      (qa (let ((text-quoting-style 'straight))
            (equal (qa123-load "(setq qa123-ran t)\n") (qa123-rejection))) t)
      (qa (let ((text-quoting-style 'grave))
            (equal (qa123-load "(setq qa123-ran t)\n") (qa123-rejection))) t)
      (qa (qa123-load (concat qa123-magic "(setq qa123-ran t)\n")) (qa123-rejection))
      (qa (qa123-load ";;; in Emacs version 31.1") (qa123-rejection))
      (qa (qa123-load (concat qa123-header "(setq qa123-ran t)\n")) (qa123-rejection))
      (qa (qa123-load (concat qa123-magic qa123-header "(setq qa123-ran t)\n")) '(t t))
      (qa (qa123-load ";comment\n;;; in Emacs version 31.1\n(setq qa123-ran t)\n") '(t t))
      (qa (qa123-load ";comment\n;;; bytecomp version FSF 31.1\n(setq qa123-ran t)\n") '(t t))
      (qa (let ((case-fold-search nil))
            (qa123-load ";comment\n;;; IN EMACS VERSION 31.1\n(setq qa123-ran t)\n")) '(t t))
      (qa (qa123-load (concat ";" (make-string 511 ?x) "\n" qa123-header "(setq qa123-ran t)\n"))
          (qa123-rejection))
      (qa (qa123-load
           (concat qa123-magic ";"
                   (make-string (- 512 (length qa123-magic) (length ";;; in Emacs version") 2) ?x)
                   "\n" qa123-header "(setq qa123-ran t)\n")) '(t t))
      (qa (qa123-load
           (concat qa123-magic ";"
                   (make-string (- 513 (length qa123-magic) (length ";;; in Emacs version") 2) ?x)
                   "\n" qa123-header "(setq qa123-ran t)\n")) (qa123-rejection))
      (qa (let ((bytecomp-version-regexp "^;;; CUSTOM"))
            (qa123-load ";comment\n;;; custom\n(setq qa123-ran t)\n")) '(t t))
      (qa (let ((bytecomp-version-regexp "^;;; CUSTOM"))
            (qa123-load (concat qa123-magic qa123-header "(setq qa123-ran t)\n"))) (qa123-rejection))
      ;; Non-string regexps abort GNU's C matcher on a nonempty header;
      ;; only exercise them when the matcher is bypassed below.
      (qa (let ((bytecomp-version-regexp "["))
            (car (qa123-load "\n(setq qa123-ran t)\n"))) 'invalid-regexp)
      (qa (let ((bytecomp-version-regexp nil)) (qa123-load "")) '(t nil))
      (qa (let ((bytecomp-version-regexp nil)) (qa123-load ";no newline")) (qa123-rejection))
      (qa (let ((qa123-file (expand-file-name "good.elc.gz" qa123-root)))
            (qa123-load (base64-decode-string "H4sIAAAAAAAC/7N29XGWZ2Bg4LK2tlbIzFNwzU1MLlYoSy0qzszPUzA21DPk0ihOLSlUKEw0NDLWLUrMUyjR5AIAFTiSKDYAAAA="))) '(t t))
      (qa (let ((qa123-file (expand-file-name "bad.elc.gz" qa123-root)))
            (qa123-load (base64-decode-string "H4sIAAAAAAAC/9MoTi0pVChMNDQy1i1KzFMo0eQCAO3ToMUTAAAA"))) '(t t))
      ;; A compiled header is also recognized on a source-named file.
      (qa (qa123-load
           (concat qa123-magic qa123-header
                   "(setq qa123-ran (list (symbolp standard-input) (bufferp standard-input)))\n"))
          '(t (t nil)))
      (qa (qa123-load
           ";comment\n;;; in Emacs version 31.1\n(setq qa123-ran (list (symbolp standard-input) (bufferp standard-input)))\n"
           (expand-file-name "header.el" qa123-root)) '(t (t nil)))
      (qa (qa123-load
           ";;; -*- lexical-binding: t; -*-\n(setq qa123-ran (list (symbolp standard-input) (bufferp standard-input)))\n"
           (expand-file-name "source.el" qa123-root)) '(t (nil t)))
      (qa (qa123-load ";;; -*- lexical-binding: t; -*-\n(setq qa123-ran t)\n"
                      (expand-file-name "module.ELC" qa123-root)) '(t t))
      (qa (let ((qa123-ran nil))
            (qa123-write "(setq qa123-ran t)\n")
            (condition-case error
                (load (file-name-sans-extension qa123-file) nil t)
              (error (list (car error) (cdr error) qa123-ran)))) (qa123-rejection))
      ;; Ordinary source files remain executable and keep match data intact.
      (qa (qa123-load ";;; -*- lexical-binding: t; -*-\n(setq qa123-ran t)\n"
                      (expand-file-name "module.el" qa123-root)) '(t t))
      (qa (let* ((before (progn (string-match "\\(b\\)" "abc") (match-data)))
                 (result (qa123-load (concat qa123-magic qa123-header "(setq qa123-ran t)\n"))))
            (list result (equal before (match-data)))) '((t t) t))
      ;; Validation precedes the custom reader and does not publish history.
      (qa (let ((load-read-function (lambda (&rest _) (error "reader ran"))))
            (qa123-load "(setq qa123-ran t)\n")) (qa123-rejection))
      (qa (let ((load-history nil))
            (qa123-load "(setq qa123-ran t)\n") load-history) nil)
      (qa (let ((after-load-functions (list (lambda (&rest _) (setq qa123-ran 'hook)))))
            (qa123-load "(setq qa123-ran t)\n")) (qa123-rejection)))
  (delete-directory qa123-root t))
(princ (qa--done))
