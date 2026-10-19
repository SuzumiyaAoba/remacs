;;; -*- lexical-binding: t; -*-
;; Issue #92: nil reader arguments use standard-input, and explicit
;; reads consume the same source as the surrounding load loop.
(defvar qa92-suffix)
(defvar qa92-inner-suffix)
(defvar qa92-value)
(defvar qa92-more)
(defvar qa92-after)
(defvar qa92-eaten)
(defvar qa92-inner)
(defvar qa92-kind)
(defvar qa92-events)
(defun qa92-load (body suffix)
  (let* ((file (make-temp-file "qa92-" nil suffix))
         (source (concat
                  (if (equal suffix ".elc")
                      (concat ";ELC" (string 31 0 0 0)
                              "\n;;; Compiled\n;;; in Emacs version 31.1\n")
                    ";;; -*- lexical-binding: t; -*-\n;; 日本語\n")
                  body)))
    (unwind-protect
        (progn (with-temp-file file (insert source)) (load file nil t) source)
      (delete-file file))))

(dolist (qa92-suffix '(".el" ".elc"))
  (qa (let (qa92-value qa92-after qa92-eaten)
        (qa92-load "(setq qa92-value (read nil))\n(setq qa92-eaten t)\n(setq qa92-after t)\n" qa92-suffix)
        (list qa92-value qa92-eaten qa92-after)) '((setq qa92-eaten t) nil t))
  (qa (let (qa92-value)
        (qa92-load "(setq qa92-value (read))\n(:datum 1)\n" qa92-suffix)
        qa92-value) '(:datum 1))
  (qa (let (qa92-value)
        (qa92-load "(eval-when-compile (setq qa92-value (read nil)))\n(:expanded)\n" qa92-suffix)
        qa92-value) '(:expanded))
  (qa (let (qa92-value)
        (qa92-load "(setq qa92-value (with-temp-buffer (insert \"(wrong)\") (goto-char 1) (list (read nil) (buffer-string) (point))))\n(:datum 2)\n" qa92-suffix)
        qa92-value) '((:datum 2) "(wrong)" 1))
  (qa (let (qa92-value qa92-after)
        (qa92-load "(setq qa92-value (let ((standard-input \"(bound)\")) (read nil)))\n(setq qa92-after t)\n" qa92-suffix)
        (list qa92-value qa92-after)) '((bound) t))
  (qa (let (qa92-value)
        (let* ((source (qa92-load "(setq qa92-value (with-temp-buffer (read-positioning-symbols nil)))\n(日本語 a)\n" qa92-suffix))
               (positions (mapcar #'symbol-with-pos-pos qa92-value))
               (start (string-match "(日本語 a)" source)))
          (list (mapcar #'bare-symbol qa92-value)
                (if (equal qa92-suffix ".el")
                    (equal positions (list (+ start 2) (+ start 6)))
                  (equal positions '(2 6)))))) '((日本語 a) t))
  (qa (let (qa92-value qa92-more)
        (qa92-load "(setq qa92-value (read-positioning-symbols))\n(foo)\n(setq qa92-more (read nil))\n(bar)\n" qa92-suffix)
        (list (symbol-with-pos-p (car qa92-value)) (bare-symbol-p (car qa92-more)))) '(t t))
  (qa (let (qa92-events qa92-value
            (load-read-function
             (lambda (stream)
               (push (eq stream standard-input) qa92-events)
               (read nil))))
        (qa92-load "(setq qa92-value 1)\n(setq qa92-value (1+ qa92-value))\n" qa92-suffix)
        (list qa92-value qa92-events)) '(2 (t t)))
  (qa (let (qa92-events (load-read-function 'read))
        (qa92-load
         (concat "(push :default-1 qa92-events)\n"
                 "(setq load-read-function (lambda (_) (setq load-read-function 'read) (read nil)))\n"
                 "(push :custom-1 qa92-events)\n(push :default-2 qa92-events)\n"
                 "(setq load-read-function (lambda (_) (setq load-read-function 'read) (read nil)))\n"
                 "(push :custom-2 qa92-events)\n(push :default-3 qa92-events)\n") qa92-suffix)
        (nreverse qa92-events)) '(:default-1 :custom-1 :default-2 :custom-2 :default-3))
  (qa (let ((old-input standard-input))
        (condition-case nil (qa92-load "(read nil)\n" qa92-suffix) (end-of-file nil))
        (eq standard-input old-input)) t)
  (qa (condition-case error (qa92-load "(read nil)\n" qa92-suffix)
        (end-of-file (list (car error) (bufferp (cadr error)) (stringp (cadr error)))))
      (if (equal qa92-suffix ".el") '(end-of-file t nil) '(end-of-file nil t)))
  (qa (condition-case error (qa92-load "(read-positioning-symbols nil)\n" qa92-suffix)
        (end-of-file (list (car error) (bufferp (cadr error)) (stringp (cadr error)))))
      (if (equal qa92-suffix ".el") '(end-of-file t nil) '(end-of-file nil t)))
  (dolist (qa92-inner-suffix '(".el" ".elc"))
    (qa (let (qa92-value qa92-inner qa92-more)
          (qa92-load
           (concat "(setq qa92-value (read nil))\n(:outer 1)\n(qa92-load "
                   (prin1-to-string "(setq qa92-inner (read nil))\n(:inner)\n")
                   " " (prin1-to-string qa92-inner-suffix) ")\n"
                   "(setq qa92-more (read nil))\n(:outer 2)\n") qa92-suffix)
          (list qa92-value qa92-inner qa92-more)) '((:outer 1) (:inner) (:outer 2)))))

(qa (let ((standard-input "(bound)")) (list (read nil) (read))) '((bound) (bound)))
(qa (let ((standard-input "(a b)"))
      (mapcar #'symbol-with-pos-pos (read-positioning-symbols nil))) '(1 3))
(qa (with-temp-buffer
      (insert "1 2") (goto-char 1)
      (let ((standard-input (current-buffer))) (list (read nil) (read) (point)))) '(1 2 4))
(qa (with-temp-buffer
      (insert "1 2") (goto-char 1)
      (let* ((marker (point-marker)) (standard-input marker))
        (list (read nil) (read) (marker-position marker) (point)))) '(1 2 4 1))
(qa (with-temp-buffer
      (insert "skip (a b)") (goto-char 6)
      (let ((standard-input (point-marker)))
        (mapcar #'symbol-with-pos-pos (read-positioning-symbols nil)))) '(1 3))
(qa (let (qa92-kind)
      (qa92-load "(setq qa92-kind (list (symbolp standard-input) (symbol-name standard-input) (fboundp standard-input) (eq standard-input (intern \"get-file-char\"))))\n" ".elc")
      qa92-kind) '(t "get-file-char" nil nil))

(princ (qa--done)) (terpri)
