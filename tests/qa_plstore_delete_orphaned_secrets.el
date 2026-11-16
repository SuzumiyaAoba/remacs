;;; qa_plstore_delete_orphaned_secrets.el --- plstore deletion regression -*- lexical-binding: t -*-

(require 'cl-lib)
(require 'plstore)

(qa
 (cl-letf (((symbol-function 'epg-make-context)
            (lambda (&optional _) 'dummy))
           ((symbol-function 'epg-context-set-passphrase-callback)
            (lambda (&rest _) nil))
           ((symbol-function 'epg-context-set-progress-callback)
            (lambda (&rest _) nil))
           ((symbol-function 'epg-decrypt-string)
            (lambda (&rest _)
              (prin1-to-string
               '(("delete" :password "gone")
                 ("keep" :token "kept"))))))
   (let* ((alist '(("delete" :secret-password t)
                   ("keep" :secret-token t)))
          (store (plstore--make nil alist "ciphertext" nil
                                (copy-tree alist))))
     (plstore-delete store "delete")
     (let ((entry (plstore-get store "keep")))
       (list (plist-get (cdr entry) :token)
             (mapcar #'car (plstore--get-secret-alist store))
             (plstore--get-encrypted-data store)))))
 '("kept" ("keep") nil))

(princ (qa--done))
