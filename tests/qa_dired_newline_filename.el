;;; -*- lexical-binding: nil; -*-
;; Dired file-name extraction must span a literal newline in a filename (#575).
(require 'dired)
(qa
 (let* ((dir (make-temp-file "dired-newline-" t))
        (file (expand-file-name "line\nbreak" dir))
        (buf nil))
   (unwind-protect
       (progn
         (write-region "" nil file)
         (setq buf (dired-noselect dir))
         (with-current-buffer buf
           (goto-char (point-min))
           (forward-line 3)
           (equal (dired-get-filename nil t) file)))
     (when (buffer-live-p buf) (kill-buffer buf))
     (delete-directory dir t)))
 t)
(princ (qa--done))
