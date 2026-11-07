;;; GNU Easy Menu suffix forms and raw menu-item properties. -*- lexical-binding: nil; -*-
(require 'easymenu)
(set-frame-parameter nil 'window-system 'ns)
(setq menu-bar-update-hook nil activate-menubar-hook nil
      qa-suffix-on t qa-raw-suffix-calls 0)
(let ((map (make-sparse-keymap)))
  (define-key map [mode]
    '(menu-item "Mode" ignore
       :suffix (progn
                 (setq qa-raw-suffix-calls (1+ qa-raw-suffix-calls))
                 (if qa-suffix-on " (on)" " (off)"))))
  (define-key global-map [menu-bar qa-raw-suffix] (cons "QA Raw Suffix" map)))
(define-key global-map [menu-bar qa-easy-suffix]
  (cons "QA Easy Suffix"
        (easy-menu-create-menu "QA Easy Suffix"
          '(["Mode" ignore :suffix (if qa-suffix-on "(on)" "(off)")]
            ["Static" ignore :suffix "fixed"]
            ["Dynamic" ignore :label (if qa-suffix-on "Label on" "Label off")
                       :suffix (if qa-suffix-on "+" "-")]))))
(menu-bar-mode 1)
