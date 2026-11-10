;;; qa_url_file_host.el --- file URL host classification regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/url-handlers.el"
       (file-name-directory load-file-name)))
(load (expand-file-name
       "../lisp/url-file.el"
       (file-name-directory load-file-name)))

(qa
 (let ((url "file://remote.example/share/docs/file.xml"))
   (list (url-handler-unhandled-file-name-directory url)
         (url-handler-file-remote-p url 'method)
         (url-handler-file-remote-p url 'host)
         (url-handler-file-remote-p url 'user)
         (url-handler-file-remote-p url 'localname)))
 '(nil "ftp" "remote.example" "anonymous" "/share/docs/file.xml"))

(qa
 (let ((url "file://localhost/share/docs/file.xml"))
   (list (url-handler-file-remote-p url 'method)
         (url-handler-file-remote-p url 'host)))
 '(nil nil))

(qa
 (url-file-build-filename "file://remote.example/share/docs/file.xml")
 "/anonymous@remote.example:/share/docs/file.xml")

(princ (qa--done))
