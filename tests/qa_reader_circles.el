;;; -*- lexical-binding: t; -*-
;; Issue #94: numbered objects preserve GNU's cons-placeholder identity,
;; including nested labels and reuse of an already defined label number.
(defvar qa94-reader 'read)
(defun qa94-read (source)
  (let ((read-circle t))
    (if (eq qa94-reader 'read-from-string)
        (car (read-from-string source))
      (funcall qa94-reader source))))

(dolist (qa94-reader '(read read-from-string read-positioning-symbols))
  (qa (let ((value (qa94-read "#1=(1 . #1#)")))
        (eq value (cdr value))) t)
  (qa (let ((value (qa94-read "#1=(#1#)")))
        (eq value (car value))) t)
  (qa (let ((value (qa94-read "#1=[1 2 #1#]")))
        (eq value (aref value 2))) t)
  (qa (let ((value (qa94-read "#1=#s(qa94 1 #1#)")))
        (eq value (aref value 2))) t)
  (qa (let* ((value (qa94-read "#1=(1 . #2=[#1# #2#])"))
             (vector (cdr value)))
        (list (eq value (aref vector 0)) (eq vector (aref vector 1)))) '(t t))
  (qa (let ((value (qa94-read "(#1=(1) #1# #2=[2] #2#)")))
        (list (eq (nth 0 value) (nth 1 value))
              (eq (nth 2 value) (nth 3 value)))) '(t t))
  ;; A new definition must replace the old mapping before its body reads.
  (qa (let* ((value (qa94-read "(#1=(1) #1=(2 . #1#) #1#)"))
             (old (nth 0 value)) (new (nth 1 value)))
        (list (eq new (cdr new)) (eq old (cdr new))
              (eq new (nth 2 value)))) '(t nil t))
  (qa (let* ((value (qa94-read "(#1=[1] #1=[2 #1#] #1#)"))
             (old (nth 0 value)) (new (nth 1 value)))
        (list (eq new (aref new 1)) (eq old (aref new 1))
              (eq new (nth 2 value)))) '(t nil t))
  ;; GNU copies the read cons into the outer placeholder, so nested cons
  ;; labels do not become aliases. Non-cons labels use substitution.
  (qa (let ((value (qa94-read "(#1=#2=(1) #1# #2#)")))
        (list (eq (nth 0 value) (nth 1 value))
              (eq (nth 0 value) (nth 2 value)))) '(t nil))
  (qa (let ((value (qa94-read "(#1=#2=[1] #1# #2#)")))
        (list (eq (nth 0 value) (nth 1 value))
              (eq (nth 0 value) (nth 2 value)))) '(t t))
  (qa (let* ((value (qa94-read "(#1=(#2=#1#) #2#)"))
             (outer (car value)) (inner (cadr value)))
        (list (eq outer (car outer)) (eq outer inner)
              (eq (car outer) inner) (car inner) (cdr inner))) '(nil nil t nil nil))
  ;; Completing an outer cons must retain the nested label's mapping.
  (qa (let* ((value (qa94-read "(#1=(#1# #1=(2 . #1#) #1#) #1#)"))
             (outer (car value)) (inner (nth 1 outer)))
        (list (eq outer (car outer)) (eq inner (cdr inner))
              (eq inner (nth 2 outer)) (eq inner (cadr value)))) '(t t t t))
  (qa (let ((value (qa94-read "#1=[#1# #1=[#1#] #1#]")))
        (list (eq value (aref value 0))
              (eq (aref value 1) (aref (aref value 1) 0))
              (eq (aref value 1) (aref value 2)))) '(t t t))
  (qa-err-data (qa94-read "#1=#1#") 'invalid-read-syntax '("nonsensical self-reference"))
  ;; Label scope ends with each read call.
  (qa-err-data (progn (qa94-read "#1=(1)") (qa94-read "#1#"))
               'invalid-read-syntax '("#1#")))

;; The other original report examples already agree with GNU: circular
;; vectors are valid when enabled, this source ends at 8, and printing a
;; circular cdr is finite even with print-circle nil.
(qa (let ((read-circle t)) (read-from-string "#1=(a b)")) '((a b) . 8))
(qa (let ((read-circle nil)) (read "[a b]")) [a b])
(qa-err-data (let ((read-circle nil)) (read "#1=[a b #1#]"))
             'invalid-read-syntax '("#1="))
(qa (let ((print-circle t)) (prin1-to-string (qa94-read "#1=(1 . #1#)")))
    "#1=(1 . #1#)")
(qa (let ((print-circle nil)) (prin1-to-string (qa94-read "#1=(1 . #1#)")))
    "(1 . #0)")

(princ (qa--done)) (terpri)
