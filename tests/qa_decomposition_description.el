;;; -*- lexical-binding: nil; -*-
;; Issue #90: use mapconcat's sequence and element validation for the
;; Unicode decomposition description bytecode.
(dolist (value (list 'font 'compat t 1 1.5 (make-marker)
                     (make-hash-table) #'car))
  (qa-err-data (char-code-property-description 'decomposition value)
               'wrong-type-argument (list 'sequencep value)))
(qa (char-code-property-description 'decomposition nil) "")
(qa (char-code-property-description 'decomposition []) "")
(qa (char-code-property-description 'decomposition "") "")
(qa (char-code-property-description 'decomposition '(font)) "font")
(qa (char-code-property-description 'decomposition '(font compat nil t)) "font compat nil t")
(qa (char-code-property-description 'decomposition [font compat]) "font compat")
(qa (char-code-property-description 'decomposition (make-bool-vector 2 t)) "t t")
(qa (char-code-property-description 'decomposition (make-bool-vector 2 nil)) "nil nil")
(qa (substring-no-properties (char-code-property-description 'decomposition '(font 65 66)))
    "font 'A' 'B'")
(qa (substring-no-properties (char-code-property-description 'decomposition [font 65 66]))
    "font 'A' 'B'")
(qa (substring-no-properties (char-code-property-description 'decomposition "AB")) "'A' 'B'")
(qa (equal-including-properties
     (char-code-property-description 'decomposition '(font 65))
     (concat "font '" (compose-string "A" 0 1 "\tA\t") "'")) t)
(qa (get-text-property 6 'composition
                       (char-code-property-description 'decomposition [font 65]))
    '((1 . "\tA\t")))
(dolist (character '(0 255 256 1114111 4194303))
  (qa (aref (char-code-property-description 'decomposition (list character)) 1) character))
(qa-err-data (char-code-property-description 'decomposition '(65 . 66))
             'wrong-type-argument '(listp 66))
(dolist (value '("a" -1 4194304 1.0))
  (qa-err-data (char-code-property-description 'decomposition (list value))
               'wrong-type-argument (list 'characterp value)))
(qa (char-code-property-description 'general-category 'Lu) "Letter, Uppercase")
(qa (char-code-property-description 'bidi-class 'L) "Left-to-Right")
(qa (char-code-property-description 'no-such-property 'font) nil)
;; An ordinary bytecode function starting with mapconcat must also keep
;; its own mapping function and separator, not be treated as unidata.
(qa (funcall #[257 "\300\301\2\302#\207" [mapconcat symbol-name ","] 5]
             '(font compat)) "font,compat")
(princ (qa--done))
