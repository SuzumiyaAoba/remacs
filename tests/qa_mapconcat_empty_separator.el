;;; -*- lexical-binding: nil; -*-
;; Empty multibyte separators do not promote mapconcat/string-join results (#556).
(let ((separator (make-string 0 ?a t)))
  (qa (list (multibyte-string-p (mapconcat #'identity '("a" "b") separator))
            (multibyte-string-p (mapconcat #'identity '("" "") separator)))
      '(nil nil))
  (qa (list (multibyte-string-p (string-join '("a" "b") separator))
            (multibyte-string-p (string-join '("" "") separator)))
      '(nil nil)))

;; Empty separators are omitted, while nonempty ones still affect the result.
(qa (multibyte-string-p (mapconcat #'identity '("a" "b") "-")) nil)
(qa (multibyte-string-p (mapconcat #'identity '("a" "b") (string-to-multibyte "-"))) t)
(qa-err (mapconcat #'identity '("a" "b") (make-bool-vector 1 t))
        'wrong-type-argument)
(qa-err (string-join '("a" "b") (make-bool-vector 1 t))
        'wrong-type-argument)
(princ (qa--done))
