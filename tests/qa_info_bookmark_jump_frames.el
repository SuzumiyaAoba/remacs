;;; -*- lexical-binding: t; -*-
;; Info bookmark lookup must not display a temporary Info buffer (#1987).

(require 'bookmark)
(require 'info)

(qa
 (let* ((file (make-temp-file "remacs-info-bookmark-"))
        (record (list "info bookmark"
                      (cons 'filename file)
                      (cons 'info-node "Top")
                      (cons 'handler #'Info-bookmark-jump)))
        (display-buffer-calls 0)
        (frames (frame-list))
        displayed-buffer)
   (unwind-protect
       (progn
         (with-temp-file file
           (insert "\x1f\nFile: qa-info-bookmark, Node: Top\n\nBookmark node.\n"))
         ;; The final bookmark display is intercepted below.  Any call to
         ;; `display-buffer' therefore comes from temporary node lookup.
         (cl-letf (((symbol-function 'display-buffer)
                    (lambda (&rest _args)
                      (setq display-buffer-calls (1+ display-buffer-calls))
                      (selected-window))))
           (bookmark-jump record
                          (lambda (buffer)
                            (setq displayed-buffer buffer))))
         (list (= display-buffer-calls 0)
               (equal frames (frame-list))
               (equal (buffer-name displayed-buffer) "*info*")
               (with-current-buffer displayed-buffer
                 (equal Info-current-node "Top"))))
     (when (buffer-live-p (get-buffer "*info*"))
       (kill-buffer "*info*"))
     (when (file-exists-p file)
       (delete-file file))))
 '(t t t t))

(princ (qa--done))
