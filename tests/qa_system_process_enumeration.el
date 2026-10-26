;;; -*- lexical-binding: nil; -*-
;; Native process enumeration and unavailable-query behavior (#609).
(let* ((pids (list-system-processes)) (snapshot (copy-sequence pids)))
  (qa (listp pids) t)
  (qa (catch 'invalid
        (dolist (pid pids)
          (unless (and (integerp pid) (>= pid 0)) (throw 'invalid nil)))
        t) t)
  (qa (= (length pids) (length (delete-dups (copy-sequence pids)))) t)
  (when pids
    (qa (not (null (memq (emacs-pid) pids))) t)
    (when (eq system-type 'darwin)
      (qa (not (null (memq 0 pids))) t)
      (qa (not (null (memq 1 pids))) t)))
  ;; A subsequent native query must not alter the previous owned snapshot.
  (list-system-processes)
  (qa (equal pids snapshot) t))
(princ (qa--done))
