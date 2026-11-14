;;; qa_gnus_uu_post_make_mime.el --- Preserve message narrowing -*- lexical-binding: t; -*-

(defvar gnus-message-buffer nil)
(defvar gnus-uu-ext-to-mime-list nil)
(defvar mail-header-separator "--text follows this line--")

(defun gnus-uu-choose-action (_file-name _file-action-list)
  "application/octet-stream")

(defun rfc2231-encode-string (_name value)
  value)

(defun mail-fetch-field (_field-name)
  nil)

(let ((gnus-uu-file
       (expand-file-name "../lisp/gnus-uu.el"
                         (file-name-directory load-file-name))))
  (with-temp-buffer
    (insert-file-contents gnus-uu-file)
    (goto-char (point-min))
    (search-forward "(defun gnus-uu-post-make-mime")
    (backward-char (length "(defun gnus-uu-post-make-mime"))
    (eval (read (current-buffer)))))

(qa
 (let* ((message-buffer (generate-new-buffer " *gnus-uu-narrowing*"))
        (gnus-message-buffer message-buffer)
        (mail-header-separator "--text follows this line--")
        (gnus-uu-ext-to-mime-list nil))
   (unwind-protect
       (progn
         (with-current-buffer message-buffer
           (insert "To: user@example.test\nSubject: test\n\n--text follows this line--\nbody\n")
           (goto-char (point-min))
           (search-forward "body")
           (backward-char 4)
           (narrow-to-region (point) (point-max)))
         (with-temp-buffer
           (gnus-uu-post-make-mime "attachment.txt" "base64"))
         (with-current-buffer message-buffer
           (let ((narrowed-text
                  (buffer-substring-no-properties (point-min) (point-max))))
             (save-restriction
               (widen)
               (and (equal narrowed-text "body\n")
                     (save-excursion
                       (goto-char (point-min))
                       (and (search-forward "MIME-Version: 1.0" nil t)
                            t))))))
       )
     (when (buffer-live-p message-buffer)
       (kill-buffer message-buffer))))
 t)

(princ (qa--done))
