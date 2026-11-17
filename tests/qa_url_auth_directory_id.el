;;; qa_url_auth_directory_id.el --- prefer specific Digest credentials -*- lexical-binding: t; -*-

;; Load only the target definition: url-auth.el has an unrelated malformed
;; form later in this checkout, and its other top-level forms are irrelevant.
(let ((source-file (expand-file-name "../lisp/url-auth.el"
                                     (file-name-directory load-file-name))))
  (with-temp-buffer
    (insert-file-contents source-file)
    (unless (re-search-forward "^(defun url-digest-auth-directory-id-assoc\\_>"
                               nil t)
      (error "Target definition missing from %s" source-file))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(qa (url-digest-auth-directory-id-assoc
     '(directory . "/private/sub/")
     '(((directory . "/") . "ROOT-CREDENTIAL")
       ((directory . "/private/") . "PRIVATE-CREDENTIAL")))
    '((directory . "/private/") . "PRIVATE-CREDENTIAL"))

;; Selection is independent of the insertion order of cached directory keys.
(qa (url-digest-auth-directory-id-assoc
     '(directory . "/private/sub/")
     '(((directory . "/private/") . "PRIVATE-CREDENTIAL")
       ((directory . "/") . "ROOT-CREDENTIAL")))
    '((directory . "/private/") . "PRIVATE-CREDENTIAL"))

;; An exact entry still outranks inherited matches.
(qa (url-digest-auth-directory-id-assoc
     '(directory . "/private/sub/")
     '(((directory . "/") . "ROOT-CREDENTIAL")
       ((directory . "/private/") . "PRIVATE-CREDENTIAL")
       ((directory . "/private/sub/") . "SUB-CREDENTIAL")))
    '((directory . "/private/sub/") . "SUB-CREDENTIAL"))

;; Realm keys match exactly and do not inherit from directory keys.
(qa (url-digest-auth-directory-id-assoc
     '(realm . "private")
     '(((realm . "root") . "ROOT-CREDENTIAL")))
    nil)

(princ (qa--done))
