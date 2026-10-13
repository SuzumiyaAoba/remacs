;; Emit the list of (SUBR-NAME . (MIN . MAX)) for every real C-level
;; subr fbound in this Emacs, one per line, as a Lisp list.  Native-
;; compiled Lisp functions also satisfy `subrp' under GNU's nativecomp
;; build; they are excluded via `subr-native-elisp-p' so the sweep
;; covers primitive operations only.  Run under both GNU and remacs;
;; the two outputs are merged offline.
(let ((out '()))
  (mapatoms
   (lambda (s)
     (let ((f (symbol-function s)))
       (when (and (subrp f)
                  (not (and (fboundp 'subr-native-elisp-p)
                            (subr-native-elisp-p f))))
         (let ((a (subr-arity f)))
           (push (cons (symbol-name s)
                       (cons (car a)
                             (if (eq (cdr a) 'many)
                                 'many
                               (cdr a))))
                 out))))))
  (prin1 (sort out (lambda (a b) (string< (car a) (car b))))))
