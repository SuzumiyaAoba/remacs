;;; -*- lexical-binding: t; -*-
;; `diff-kill-ring-save' rejects source context changed after the diff (#1983).

(let* ((lisp-directory
        (expand-file-name "../lisp" (file-name-directory load-file-name)))
       (load-path (cons lisp-directory load-path)))
  (load (expand-file-name "diff-mode.el" lisp-directory) nil t t))

(qa
 (let* ((file (make-temp-file "qa-diff-kill-ring-stale-"))
        (source-buffer nil)
        (diff-buffer (generate-new-buffer " *qa-diff-kill-ring-stale*"))
        (diff-text nil)
        (kill-ring '("sentinel"))
        (original-copy nil)
        (stale-rejected nil)
        (ring-preserved nil))
   (unwind-protect
       (progn
         (with-temp-file file
           (insert "old-1\nline-2\nline-3\nline-4\nline-5\nline-6\nline-7\nline-8\nold-9\n"))
         (setq source-buffer (find-file-noselect file))
         (setq diff-text
               (format
                (concat "--- %s\n+++ %s\n"
                        "@@ -1,4 +1,4 @@\n"
                        "-old-1\n+new-1\n line-2\n line-3\n line-4\n"
                        "@@ -6,4 +6,4 @@\n"
                        " line-6\n line-7\n line-8\n-old-9\n+new-9\n")
                file file))
         (with-current-buffer diff-buffer
           (insert diff-text)
           (diff-mode)
           (goto-char (point-min))
           (search-forward "@@ -1,4 +1,4 @@")
           (beginning-of-line)
           (let ((beg (point))
                 (end (point-max)))
             (diff-kill-ring-save beg end)
             (setq original-copy (car kill-ring))
             (with-current-buffer source-buffer
               (goto-char (point-min))
               (forward-line 3)
               (delete-region (line-beginning-position) (line-end-position))
               (insert "stale-line-4"))
             (let ((previous-copy (car kill-ring)))
               (condition-case err
                   (diff-kill-ring-save beg end)
                 (error
                  (setq stale-rejected
                        (equal (error-message-string err)
                               "Diff is stale: source context no longer matches"))))
               (setq ring-preserved (equal (car kill-ring) previous-copy)))))
         (list (stringp original-copy) stale-rejected ring-preserved))
     (when (buffer-live-p source-buffer)
       (kill-buffer source-buffer))
     (when (buffer-live-p diff-buffer)
       (kill-buffer diff-buffer))
     (when (file-exists-p file)
       (delete-file file))))
 '(t t t))

(princ (qa--done))
