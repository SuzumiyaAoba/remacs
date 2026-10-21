;;; -*- lexical-binding: t; -*-
;; Issue #237: SOURCE buffers contribute only their accessible characters.
(defun qa237-replace (kind seconds costs)
  (let ((source (generate-new-buffer " *qa237-source*")))
    (unwind-protect
        (progn
          (with-current-buffer source
            (insert "xxNEWyy") (narrow-to-region 3 6) (goto-char 4))
          (with-temp-buffer
            (insert "preoldpost") (narrow-to-region 2 9)
            (let* ((target (current-buffer))
                   (result (replace-region-contents
                            4 7
                            (cond ((eq kind 'buffer) source)
                                  ((eq kind 'vector) (vector source 3 6))
                                  ((eq kind 'callback) (lambda () source)))
                            seconds costs)))
              (list result (buffer-string) (eq target (current-buffer))
                    (point-min) (point-max)
                    (with-current-buffer source
                      (list (buffer-string) (point) (point-min) (point-max)
                            (save-restriction (widen) (buffer-string))))))))
      (kill-buffer source))))

(qa
 (let ((source (generate-new-buffer " *qa237-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "xxNEWyy") (narrow-to-region 3 6))
	 (with-temp-buffer
	   (insert "old") (replace-region-contents 1 4 source)
	   (buffer-string)))
     (kill-buffer source)))
 '"NEW")

(qa (qa237-replace 'buffer nil nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'buffer 0 nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'buffer -1 nil)
    '(nil "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'vector nil nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'vector 0 nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'vector -1 nil)
    '(nil "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'callback nil nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'callback 0 nil)
    '(t "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa (qa237-replace 'callback -1 nil)
    '(nil "reNEWpo" t 2 9 ("NEW" 4 3 6 "xxNEWyy")))

(qa
 (let ((source (generate-new-buffer " *qa237-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "hidden") (narrow-to-region 3 3))
	 (with-temp-buffer
	   (insert "old")
	   (let ((result (replace-region-contents 1 4 source)))
	     (list result (buffer-string) (point)))))
     (kill-buffer source)))
 '(t "" 1))

(qa
 (let ((source (generate-new-buffer " *qa237-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "xxNEWyy") (narrow-to-region 3 6))
	 (with-temp-buffer
	   (replace-region-contents 1 1 source) (buffer-string)))
     (kill-buffer source)))
 '"NEW")

(qa
 (let ((source (generate-new-buffer " *qa237-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert (propertize "隠しαβ隠し" 'face 'bold))
	   (narrow-to-region 3 5))
	 (with-temp-buffer
	   (insert "old") (replace-region-contents 1 4 source)
	   (list (buffer-substring-no-properties 1 3)
		 (get-text-property 1 'face)
		 (get-text-property 2 'face))))
     (kill-buffer source)))
 '("αβ" bold bold))

(qa
 (let ((source (generate-new-buffer " *qa237-source*")))
   (unwind-protect
       (progn
	 (with-current-buffer source
	   (insert "xxNEWyy") (narrow-to-region 3 6))
	 (with-temp-buffer
	   (insert "old")
	   (let*
	       ((target (current-buffer))
		(error
		 (condition-case err
		     (replace-region-contents 1 4 (vector source 2 6))
		   (error (car err)))))
	     (list error (eq target (current-buffer)) (buffer-string)
		   (with-current-buffer source
		     (list (point-min) (point-max)))))))
     (kill-buffer source)))
 '(args-out-of-range t "old" (3 6)))


(princ (qa--done)) (terpri)
