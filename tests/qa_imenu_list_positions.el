;;; -*- lexical-binding: t -*-
(require 'imenu)

(qa (imenu--subalist-p '("item" (10 20) ignore)) nil)
(qa (imenu--subalist-p '("item" nil ignore)) nil)
(qa (imenu--subalist-p '("item" (10 . 20) ignore extra)) nil)
(qa (imenu--subalist-p (list "item" '(10 20) (lambda (&rest _) nil))) nil)
(qa (imenu--subalist-p '("item" (lambda () 10) ignore)) nil)
(qa (imenu--subalist-p '("item" 10 ignore)) nil)
(qa (imenu--subalist-p '("item" "position" ignore)) nil)
(qa (imenu--subalist-p '("item" . 10)) nil)
(qa (imenu--subalist-p '("group" ("child" . 10))) t)
(qa (imenu--subalist-p '("group" ("one" . 10) ("two" . 20))) t)
(qa (imenu--subalist-p '("group" ("nested" ("child" . 10)))) t)

(defun qa-imenu--list-position-menu (duplicates)
  (let* ((imenu-allow-duplicate-menu-items duplicates)
         (position (list 10 20))
         (item (list "item" position #'ignore 'extra))
         (map (imenu--create-keymap "Index" (list item))))
    (eq (funcall (cddr (nth 2 map))) item)))

(qa (qa-imenu--list-position-menu t) t)
(qa (qa-imenu--list-position-menu nil) t)

(qa (let* ((position (list 10 20))
           (item (list "item" position #'ignore 'extra))
           (map (imenu--create-keymap "Index" (list item)
                                      (lambda (chosen) (eq chosen item)))))
      (funcall (cddr (nth 2 map))))
    t)

(qa (let* ((original (symbol-function 'completing-read))
           (position (list 10 20))
           (item (list "item" position #'ignore 'extra)))
      (unwind-protect
          (progn
            (fset 'completing-read (lambda (&rest _) "item"))
            (with-temp-buffer
              (let ((imenu-space-replacement nil))
                (eq (imenu--completion-buffer (list item) "Pick") item))))
        (fset 'completing-read original)))
    t)

(qa (let* ((position (list 10 20))
           (item (list "item" position #'ignore 'extra)))
      (eq (cdr (car (imenu--flatten-index-alist
                     (list (cons "group" (list item))) t)))
          (cdr item)))
    t)

(qa (let* ((item (list "item" '(10 20) #'ignore 'extra))
           (index (list (cons "group" (list item)))))
      (eq (imenu--in-alist "item" index) item))
    t)

(defun qa-imenu--jump-list-position ()
  (let* ((position (list 10 20))
         received
         (item (list "item" position
                     (lambda (name target &rest args)
                       (setq received (list name (eq target position) args))
                       (goto-char (car target)))
                     'first 'second)))
    (with-temp-buffer
      (insert (make-string 30 ?x))
      (goto-char 1)
      (let ((imenu-after-jump-hook nil))
        (imenu item))
      (list received (point)))))

(qa (qa-imenu--jump-list-position) '(("item" t (first second)) 10))

(princ (qa--done))
