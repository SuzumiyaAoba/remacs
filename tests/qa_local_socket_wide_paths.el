;;; -*- lexical-binding: nil; -*-
;; #624: public Unix socket addresses are native-byte strings, not proxies.
(defun qa624-local-address-shape ()
  (let* ((directory (make-temp-file "/tmp/rm-s624-" t))
         (path (concat directory "/" (string #xf0080 #xf0100)))
         server client)
    (unwind-protect
        (progn
          (setq server (make-network-process :name "qa624-server" :family 'local
                                             :server t :service path :noquery t))
          (setq client (make-network-process :name "qa624-client" :family 'local
                                             :service path :noquery t))
          (let ((local (process-contact server :local))
                (remote (process-contact client :remote))
                (bytes (string-as-unibyte path)))
            (list (multibyte-string-p local) (equal local bytes)
                  (multibyte-string-p remote) (equal remote bytes))))
      (when (processp client) (delete-process client))
      (when (processp server) (delete-process server))
      (delete-directory directory t))))
(qa (qa624-local-address-shape) '(nil t nil t))
(princ (qa--done))
