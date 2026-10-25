;;; -*- lexical-binding: nil; -*-
;; recordp excludes opaque vector types, but uses object identity for
;; record-backed font specs and obarrays (#477).
(qa (mapcar #'recordp (list (bool-vector t)
                            (make-char-table 'test)
                            (record 'test)))
    '(nil nil t))
(qa (recordp (record 'font-spec '(:family "ordinary"))) t)
(qa (recordp (font-spec :family "opaque")) nil)
(qa (obarrayp (record 'obarray [] 3)) nil)
(qa (recordp (record 'obarray [] 3)) t)
(qa (recordp (obarray-make)) nil)
(princ (qa--done))
