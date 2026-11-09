;;; -*- lexical-binding: nil; -*-
;; Echo-area truncation is measured in display columns.
(require 'cl-lib)
(require 'eldoc)
(defun qa-eldoc--capture-echo (docs multiline)
  (let (echo-output)
    (cl-letf (((symbol-function 'window-width)
               (lambda (&optional _window) 15))
              ((symbol-function 'eldoc--message)
               (lambda (message) (setq echo-output message)))
              ((symbol-function 'eldoc--echo-area-prefer-doc-buffer-p)
               (lambda (_truncated) nil)))
      (let ((eldoc-echo-area-use-multiline-p multiline)
            (eldoc-echo-area-prefer-doc-buffer nil))
        (eldoc-display-in-echo-area docs t))
      echo-output)))
(qa (string-width (qa-eldoc--capture-echo
                   '(("ああああああああ")) nil))
    14)
;; The special case that omits a long symbol name also uses display widths.
(qa (qa-eldoc--capture-echo
     '(("a" :thing "あああああああ")) 'truncate-sym-name-if-fit)
    "a")
(princ (qa--done))
