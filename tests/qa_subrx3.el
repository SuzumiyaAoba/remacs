;;; -*- lexical-binding: t -*-
;; Loading subr-x after startup, `buffer-text-pixel-size', nativecomp
;; funcall arity — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - `(load "subr-x")' only (re)defines subr-x's own functions: other
;;     live definitions (replace.el's `replace-highlight', primitives)
;;     and settings (`electric-quote-string', `file-name-handler-alist')
;;     are untouched, and no other feature gets provided.
;;   - subr-x defines `with-buffer-unmodified-if-unchanged'.
;;   - `buffer-text-pixel-size' on the batch tty measures columns via
;;     `char-width' (glyphless `thin-space' chars take 1, control chars
;;     2), tab stops, X-LIMIT wrapping (continued lines span the limit),
;;     Y-LIMIT capping (non-fixnum Y-LIMIT measures nothing), narrowing.
;;   - `apply' drops surplus args for a native subr with more than 8
;;     parameters; a call form still signals wrong-number-of-arguments.

(defvar qasx3--rh (symbol-function 'replace-highlight))
(defvar qasx3--sha1 (symbol-function 'sha1))
(defvar qasx3--handlers (length file-name-handler-alist))
(setq electric-quote-string 'qasx3)
(load "subr-x" nil t)

;; == loading subr-x leaves the rest of the session alone ==
(qa (eq (symbol-function 'replace-highlight) qasx3--rh) t)
(qa (eq (symbol-function 'sha1) qasx3--sha1) t)
(qa electric-quote-string 'qasx3)
(qa (length file-name-handler-alist) qasx3--handlers)
(qa (featurep 'subr-x) t)
(qa (featurep 'loadhist) nil)
(qa (fboundp 'with-buffer-unmodified-if-unchanged) t)
(qa (string-pad "ab" 4 ?.) "ab..")
(qa (with-temp-buffer
      (insert "x")
      (set-buffer-modified-p nil)
      (with-buffer-unmodified-if-unchanged (insert "y") (delete-char -1))
      (buffer-modified-p))
    nil)

;; == buffer-text-pixel-size ==
(defun qasx3--btps (text &optional x y)
  (with-temp-buffer (insert text) (buffer-text-pixel-size nil nil x y)))
(qa (qasx3--btps "日" t) '(2 . 1))
(qa (qasx3--btps "a\tb" t) '(9 . 1))
(qa (qasx3--btps "ab\ncdef\ng" t) '(4 . 3))
(qa (qasx3--btps "ab\n\ncd\n" t) '(2 . 3))
(qa (qasx3--btps "\n\n" t) '(0 . 2))
(qa (qasx3--btps "" t) '(0 . 0))
(qa (qasx3--btps "abcdefgh" 3) '(3 . 3))
(qa (qasx3--btps "日本語" 3) '(3 . 3))
(qa (qasx3--btps "abc" 0) '(0 . 3))
(qa (qasx3--btps "abcdef" -1) '(6 . 1))
(qa (qasx3--btps "a\nb\nc\nd" t 2) '(1 . 2))
(qa (qasx3--btps "a\nb\nc" t t) '(1 . 0))
(qa (qasx3--btps "abc\ndefgh" t 1) '(5 . 1))
(qa (qasx3--btps "a\nbbb\ncccccc\ndddddddddd" t 1) '(3 . 1))
(qa (with-temp-buffer (insert "a\nbbb\ncc") (buffer-text-pixel-size nil nil nil 1))
    '(1 . 1))
(qa (with-temp-buffer (insert "a\nbbb\ncc") (buffer-text-pixel-size nil nil nil t))
    '(0 . 0))
(qa (qasx3--btps "ab\tc" 3) '(3 . 2))
(qa (qasx3--btps "\ta" 7) '(7 . 1))
(qa (qasx3--btps "\ta" 8) '(8 . 2))
(qa (qasx3--btps "a\tb\tc" 5) '(5 . 3))
(qa (qasx3--btps (string 1 ?a) t) '(3 . 1))
(qa (qasx3--btps (string #x200b) t) '(1 . 1))
(qa (with-temp-buffer (setq tab-width 4) (insert "a\tb") (buffer-text-pixel-size))
    '(5 . 1))
(qa (with-temp-buffer
      (insert "aaaa\nbb\ncccccc")
      (narrow-to-region 6 8)
      (buffer-text-pixel-size nil nil t))
    '(2 . 1))
(qa (string-pixel-width "日本") 4)

;; == funcall arity of native subrs with more than 8 parameters ==
(qa (type-of (apply #'cl--struct-new-class (make-list 11 nil))) 'cl-structure-class)
(qa-err (cl--struct-new-class nil nil nil nil nil nil nil nil nil nil nil)
  'wrong-number-of-arguments)
(qa (condition-case e (apply #'cl-struct-define (make-list 10 nil)) (error (car e)))
    'wrong-type-argument)
(qa (condition-case e
        (with-temp-buffer (apply #'help--describe-map-tree (make-list 11 nil)))
      (error (car e)))
    'wrong-type-argument)

(princ (qa--done))
