;;; qa_url_cookie_session_cleanup.el --- preserve session cookies during save -*- lexical-binding: t; -*-

(require 'url-cookie)

(qa
 (list
  (let ((url-cookie-storage nil)
        (url-cookie-secure-storage nil))
    (url-cookie-store "sid" "v" nil "example.com" "/")
    (url-cookie-store "secure-sid" "v" nil "secure.example.com" "/" t)
    (let ((before
           (list (length (url-cookie-retrieve "example.com" "/"))
                 (length (url-cookie-retrieve "secure.example.com" "/" t)))))
      (url-cookie-clean-up)
      (url-cookie-clean-up t)
      (list before
            (length (url-cookie-retrieve "example.com" "/"))
            (length (url-cookie-retrieve "secure.example.com" "/" t)))))
  (let* ((directory (make-temp-file "remacs-url-cookie-" t))
         (file (expand-file-name "cookies.el" directory))
         (url-cookie-storage nil)
         (url-cookie-secure-storage nil)
         (url-cookies-changed-since-last-save t)
         (url-cookie-file file))
    (unwind-protect
        (progn
          (url-cookie-store "SESSION-ONLY-PLAIN" "session" nil "example.com" "/")
          (url-cookie-store "PERSISTENT" "saved"
                            (time-add (current-time) 3600)
                            "example.com" "/")
          (url-cookie-store "SESSION-ONLY-SECURE" "secret" nil
                            "secure.example.com" "/" t)
          (setq url-cookies-changed-since-last-save t)
          (url-cookie-write-file file)
          (let ((contents (with-temp-buffer
                            (insert-file-contents file)
                            (buffer-string))))
            (list (length (cdr (assoc-string "example.com" url-cookie-storage t)))
                  (length (cdr (assoc-string "secure.example.com"
                                              url-cookie-secure-storage t)))
                  (if (string-match-p "SESSION-ONLY-PLAIN" contents) t nil)
                  (if (string-match-p "SESSION-ONLY-SECURE" contents) t nil)
                  (if (string-match-p "PERSISTENT" contents) t nil))))
      (delete-directory directory t))))
 '(((1 1) 1 1) (2 1 nil nil t)))

(princ (qa--done))
