;;; qa_multisession_files_case.el --- multisession case-distinct files regression -*- lexical-binding: t -*-

(require 'multisession)

(defun qa-multisession-case-distinct-files-result ()
  (let* ((directory (make-temp-file "multisession-case-probe-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory)
         (upper (make-multisession :key "Key" :package "probe"
                                   :initial-value "initial" :storage 'files))
         (lower (make-multisession :key "key" :package "probe"
                                   :initial-value "initial" :storage 'files))
         (read-upper (make-multisession :key "Key" :package "probe"
                                        :initial-value "initial" :storage 'files))
         (read-lower (make-multisession :key "key" :package "probe"
                                        :initial-value "initial" :storage 'files)))
    (unwind-protect
        (progn
          (setf (multisession-value upper) "upper-value")
          (setf (multisession-value lower) "lower-value")
          (list (not (equal (downcase (multisession--object-file-name upper))
                            (downcase (multisession--object-file-name lower))))
                (multisession-value read-upper)
                (multisession-value read-lower)))
      (delete-directory directory t))))

(defun qa-multisession-legacy-uppercase-file-result ()
  (let* ((directory (make-temp-file "multisession-legacy-probe-" t))
         (user-init-file "/tmp/init.el")
         (multisession-directory directory)
         (object (make-multisession :key "Key" :package "probe"
                                    :initial-value "initial" :storage 'files))
         (legacy-file
          (expand-file-name
           (concat "files/probe/" (url-hexify-string "Key") ".value")
           directory)))
    (unwind-protect
        (progn
          (make-directory (file-name-directory legacy-file) t)
          (with-temp-file legacy-file
            (prin1 "legacy-value" (current-buffer)))
          (multisession-value object))
      (delete-directory directory t))))

(let ((expected '(t "upper-value" "lower-value")))
  (if qa--remacs
      (qa (qa-multisession-case-distinct-files-result) expected)
    (qa-xf (qa-multisession-case-distinct-files-result) expected
           "Remacs encodes case-distinct Files backend keys separately")))

(qa (qa-multisession-legacy-uppercase-file-result) "legacy-value")

(princ (qa--done))
