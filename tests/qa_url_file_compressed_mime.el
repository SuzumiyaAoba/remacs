;;; qa_url_file_compressed_mime.el --- compressed file MIME regression -*- lexical-binding:t -*-

(load (expand-file-name
       "../lisp/url-file.el"
       (file-name-directory load-file-name)))

(qa
 (url-file--uncompressed-filename "/tmp/readme.txt.xz")
 "/tmp/readme.txt")

(qa
 (url-file--uncompressed-filename "/tmp/readme.txt.bz2")
 "/tmp/readme.txt")

(qa
 (mailcap-extension-to-mime
  (url-file-extension
   (url-file--uncompressed-filename "/tmp/readme.txt.xz")))
 "text/plain")

(qa
 (mailcap-extension-to-mime
  (url-file-extension
   (url-file--uncompressed-filename "/tmp/readme.txt.bz2")))
 "text/plain")

(qa (url-file--uncompressed-filename "/tmp/readme.txt.gz") "/tmp/readme.txt")
(qa (url-file--uncompressed-filename "/tmp/readme.txt.Z") "/tmp/readme.txt")
(qa (url-file--uncompressed-filename "/tmp/readme.txt") "/tmp/readme.txt")

(princ (qa--done))
