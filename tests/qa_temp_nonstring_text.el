;;; qa_temp_nonstring_text.el --- Ignore nonstring temporary contents -*- lexical-binding: t; -*-

(defun qa331-text (internal text &optional directory)
  (with-temp-buffer
    (let* ((parent (make-temp-file "qa331-parent-" t))
           (selected nil)
           (file nil))
      (fset 'qa331-select
            (lambda (&rest _) (setq selected t) 'utf-8-unix))
      (unwind-protect
          (let ((select-safe-coding-system-function 'qa331-select))
            (setq file (if internal
                           (make-temp-file-internal (expand-file-name "child-" parent) directory "" text)
                         (make-temp-file (expand-file-name "child-" parent) directory nil text)))
            (list (stringp file) selected
                  (length (directory-files parent nil "\\`[^.]"))
                  (unless directory
                    (with-temp-buffer
                      (set-buffer-multibyte nil)
                      (insert-file-contents-literally file)
                      (string-to-list (buffer-string))))))
        (fmakunbound 'qa331-select)
        (delete-directory parent t)))))

;; nonstring initial text internal=nil
(qa (qa331-text nil nil)
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil 42)
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil t)
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil 0)
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil '(a))
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil [a])
    '(t nil 1 nil))

;; nonstring initial text internal=nil
(qa (qa331-text nil #'ignore)
    '(t nil 1 nil))

;; empty text avoids coding selection internal=nil
(qa (qa331-text nil "")
    '(t nil 1 nil))

;; empty text avoids coding selection internal=nil
(qa (qa331-text nil (string-to-multibyte ""))
    '(t nil 1 nil))

;; nonempty string still writes internal=nil
(qa (qa331-text nil "é")
    '(t t 1 (195 169)))

;; directory ignores nonstring text internal=nil
(qa (qa331-text nil 42 t)
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t nil)
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t 42)
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t t)
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t 0)
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t '(a))
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t [a])
    '(t nil 1 nil))

;; nonstring initial text internal=t
(qa (qa331-text t #'ignore)
    '(t nil 1 nil))

;; empty text avoids coding selection internal=t
(qa (qa331-text t "")
    '(t nil 1 nil))

;; empty text avoids coding selection internal=t
(qa (qa331-text t (string-to-multibyte ""))
    '(t nil 1 nil))

;; nonempty string still writes internal=t
(qa (qa331-text t "é")
    '(t t 1 (195 169)))

;; directory ignores nonstring text internal=t
(qa (qa331-text t 42 t)
    '(t nil 1 nil))

(princ (qa--done))
