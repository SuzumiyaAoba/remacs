;;; qa_delete_killed_target.el --- Protect deletions after target death -*- lexical-binding: t; -*-

(defun qa324-switch (form kill-target)
  (with-temp-buffer
    (insert "SAFE")
    (let* ((outer (current-buffer))
           (target (generate-new-buffer " *qa324-target*"))
           (result
            (with-current-buffer target
              (insert "abcd")
              (goto-char 2)
              (let ((before-change-functions
                     (list (lambda (&rest _)
                             (when kill-target (kill-buffer target))
                             (set-buffer outer)))))
                (condition-case error (eval form t) (error error))))))
      (unwind-protect
          (list result (buffer-substring-no-properties (point-min) (point-max))
                (if (buffer-live-p target)
                    (with-current-buffer target
                      (buffer-substring-no-properties (point-min) (point-max)))
                  'killed))
        (when (buffer-live-p target) (kill-buffer target))))))

;; killed target
(qa (qa324-switch '(delete-region 2 3) t)
    '((error "Marker does not point anywhere") "SAFE" killed))

;; killed target
(qa (qa324-switch '(delete-and-extract-region 2 3) t)
    '((error "Marker does not point anywhere") "SAFE" killed))

;; killed target
(qa (qa324-switch '(delete-char 1) t)
    '((error "Marker does not point anywhere") "SAFE" killed))

;; killed target
(qa (qa324-switch '(delete-backward-char 1) t)
    '((error "Marker does not point anywhere") "SAFE" killed))

;; killed target
(qa (qa324-switch '(erase-buffer) t)
    '((error "Marker does not point anywhere") "SAFE" killed))

;; killed target
(qa (qa324-switch '(insert "X") t)
    '(nil "SAFEX" killed))

;; live target
(qa (qa324-switch '(delete-region 2 3) nil)
    '(nil "SFE" "abcd"))

;; live target
(qa (qa324-switch '(delete-and-extract-region 2 3) nil)
    '("A" "SFE" "abcd"))

;; live target
(qa (qa324-switch '(delete-char 1) nil)
    '(nil "SFE" "abcd"))

;; live target
(qa (qa324-switch '(delete-backward-char 1) nil)
    '(nil "AFE" "abcd"))

;; live target
(qa (qa324-switch '(erase-buffer) nil)
    '(nil "" "abcd"))

;; live target
(qa (qa324-switch '(insert "X") nil)
    '(nil "SAFEX" "abcd"))

(qa (with-temp-buffer
      (insert "abcd")
      (let ((called nil))
        (let ((before-change-functions (list (lambda (&rest _) (setq called t)))))
          (list (delete-region 2 2) called (buffer-string)))))
    '(nil nil "abcd"))

(princ (qa--done))
