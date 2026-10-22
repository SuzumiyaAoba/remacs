;;; qa_set_file_times_flags.el --- Any non-nil flag avoids symlink traversal -*- lexical-binding: t; -*-

(defun qa329-times (flag kind)
  (let* ((parent (make-temp-file "qa329-parent-" t))
         (file (expand-file-name "file" parent))
         (link (if (eq kind 'regular) file (expand-file-name "link" parent))))
    (unwind-protect
        (progn
          (unless (eq kind 'broken)
            (write-region "x" nil file nil 'silent)
            (set-file-times file 1000000000))
          (unless (eq kind 'regular) (make-symbolic-link file link))
          (set-file-times link 900000000 'nofollow)
          (list (condition-case error (set-file-times link 1100000000 flag)
                  (error (car error)))
                (let ((attributes (file-attributes file)))
                  (and attributes
                       (list (time-convert (nth 4 attributes) 'integer)
                             (time-convert (nth 5 attributes) 'integer))))
                (let ((attributes (file-attributes link)))
                  (list (time-convert (nth 4 attributes) 'integer)
                        (time-convert (nth 5 attributes) 'integer)))))
      (delete-directory parent t))))

;; link flag nil
(qa (qa329-times nil 'link)
    '(t (1100000000 1100000000) (900000000 900000000)))

;; link flag t
(qa (qa329-times t 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag 'nofollow
(qa (qa329-times 'nofollow 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag 'other
(qa (qa329-times 'other 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag 0
(qa (qa329-times 0 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag "yes"
(qa (qa329-times "yes" 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag []
(qa (qa329-times [] 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag '(x)
(qa (qa329-times '(x) 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; link flag 1.0
(qa (qa329-times 1.0 'link)
    '(t (1000000000 1000000000) (1100000000 1100000000)))

;; broken flag nil
(qa (qa329-times nil 'broken)
    '(file-missing nil (900000000 900000000)))

;; broken flag t
(qa (qa329-times t 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag 'nofollow
(qa (qa329-times 'nofollow 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag 'other
(qa (qa329-times 'other 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag 0
(qa (qa329-times 0 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag "yes"
(qa (qa329-times "yes" 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag []
(qa (qa329-times [] 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag '(x)
(qa (qa329-times '(x) 'broken)
    '(t nil (1100000000 1100000000)))

;; broken flag 1.0
(qa (qa329-times 1.0 'broken)
    '(t nil (1100000000 1100000000)))

;; ordinary file
(qa (qa329-times nil 'regular)
    '(t (1100000000 1100000000) (1100000000 1100000000)))

;; ordinary file
(qa (qa329-times t 'regular)
    '(t (1100000000 1100000000) (1100000000 1100000000)))

(princ (qa--done))
