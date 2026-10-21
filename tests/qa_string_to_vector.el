;;; -*- lexical-binding: t; -*-
;; Issue #257: string-to-vector is vconcat, including non-string sequences.
(defun qa257-fresh (source)
  (let ((out (string-to-vector source))) (list out (eq source out))))
(defun qa257-error (source)
  (condition-case err (string-to-vector source)
    (error (list (car err) (cadr err) (eq (nth 2 err) source)))))
(defun qa257-cycle (size entry)
  (let ((source (number-sequence 0 (1- size))))
    (setcdr (last source) (nthcdr entry source))
    (condition-case err (progn (string-to-vector source) 'accepted)
      (error
       (let ((node source) (position 0) (found nil))
        (while (and (< position size) (not found))
         (if (eq node (cadr err)) (setq found position)
          (setq node (cdr node) position (1+ position))))
        (list (car err) found))))))
(qa
(qa257-fresh nil)
'([] nil)
)

(qa
(qa257-fresh (intern "nil"))
'([] nil)
)

(qa
(qa257-fresh (list 1))
'([1] nil)
)

(qa
(qa257-fresh (list 1 2))
'([1 2] nil)
)

(qa
(qa257-fresh (list 'a nil 'b))
'([a nil b] nil)
)

(qa
(qa257-fresh [])
'([] t)
)

(qa
(qa257-fresh [1 2 nil "x"])
'([1 2 nil "x"] nil)
)

(qa
(qa257-fresh (bool-vector))
'([] nil)
)

(qa
(qa257-fresh (bool-vector t nil t nil))
'([t nil t nil] nil)
)

(qa
(qa257-fresh "")
'([] nil)
)

(qa
(qa257-fresh "abc")
'([97 98 99] nil)
)

(qa
(qa257-fresh (string-make-multibyte "abc"))
'([97 98 99] nil)
)

(qa
(qa257-fresh (unibyte-string 0 127 128 255))
'([0 127 128 255] nil)
)

(qa
(qa257-fresh "日é")
'([26085 233] nil)
)

(qa
(qa257-fresh (string 55296 1114112 4194176))
'([55296 1114112 4194176] nil)
)

(qa
(qa257-fresh (propertize "ab" 'face 'bold))
'([97 98] nil)
)

(qa
(let* ((source (vector 1 2)) (out (string-to-vector source)))
  (aset out 0 9) (list source out))
'([1 2] [9 2])
)

(qa
(let*
    ((cell (list 1)) (source (vector cell))
     (out (string-to-vector source)))
  (list (eq source out) (eq (aref source 0) (aref out 0))
	(progn (setcar (aref out 0) 9) (aref source 0))))
'(nil t (9))
)

(qa
(string-to-vector (apply #'unibyte-string (number-sequence 0 255)))
'[0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25
    26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
    48 49 50 51 52 53 54 55 56 57 58 59 60 61 62 63 64 65 66 67 68 69
    70 71 72 73 74 75 76 77 78 79 80 81 82 83 84 85 86 87 88 89 90 91
    92 93 94 95 96 97 98 99 100 101 102 103 104 105 106 107 108 109
    110 111 112 113 114 115 116 117 118 119 120 121 122 123 124 125
    126 127 128 129 130 131 132 133 134 135 136 137 138 139 140 141
    142 143 144 145 146 147 148 149 150 151 152 153 154 155 156 157
    158 159 160 161 162 163 164 165 166 167 168 169 170 171 172 173
    174 175 176 177 178 179 180 181 182 183 184 185 186 187 188 189
    190 191 192 193 194 195 196 197 198 199 200 201 202 203 204 205
    206 207 208 209 210 211 212 213 214 215 216 217 218 219 220 221
    222 223 224 225 226 227 228 229 230 231 232 233 234 235 236 237
    238 239 240 241 242 243 244 245 246 247 248 249 250 251 252 253
    254 255]
)

(qa
(condition-case err (string-to-vector '(1 . 2)) (error err))
'(wrong-type-argument listp 2)
)

(qa
(condition-case err (string-to-vector '(1 2 . tail)) (error err))
'(wrong-type-argument listp tail)
)

(qa
(condition-case err (string-to-vector (cons 1 "bad")) (error err))
'(wrong-type-argument listp "bad")
)

(qa
(condition-case err (string-to-vector (cons 1 [2 3])) (error err))
'(wrong-type-argument listp [2 3])
)

(qa
(qa257-error 42)
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error 1.5)
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (expt 2 100))
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error 'bad)
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (point-marker))
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (record 'tag 1))
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (make-char-table nil))
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (symbol-function 'car))
'(wrong-type-argument sequencep t)
)

(qa
(qa257-error (make-byte-code 0 (unibyte-string 192 135) [11] 1))
'[0 "\300\207" [11] 1]
)

(qa
(qa257-error (eval '(lambda nil 11) t))
'[nil (11) (t)]
)

(qa
(qa257-error (eval '(lambda (x) (+ x 1)) t))
'[(x) ((+ x 1)) (t)]
)

(qa
(qa257-error (eval '(lambda nil "doc" (interactive) 11) t))
'[nil (11) (t) nil "doc" nil]
)

(qa
(qa257-error
 (make-byte-code 0 (unibyte-string 192 135) [11] 1 "doc"
		 '(interactive)))
'[0 "\300\207" [11] 1 "doc" (interactive)]
)

(qa
(qa257-error (eval '(lambda nil 11) nil))
'[nil (11) nil]
)

(qa
(qa257-cycle 1 0)
'(circular-list 0)
)

(qa
(qa257-cycle 2 0)
'(circular-list 0)
)

(qa
(qa257-cycle 2 1)
'(circular-list 1)
)

(qa
(qa257-cycle 3 0)
'(circular-list 2)
)

(qa
(qa257-cycle 3 1)
'(circular-list 2)
)

(qa
(qa257-cycle 3 2)
'(circular-list 2)
)

(qa
(qa257-cycle 4 0)
'(circular-list 2)
)

(qa
(qa257-cycle 4 1)
'(circular-list 2)
)

(qa
(qa257-cycle 4 2)
'(circular-list 2)
)

(qa
(qa257-cycle 4 3)
'(circular-list 3)
)

(qa
(qa257-cycle 5 0)
'(circular-list 1)
)

(qa
(qa257-cycle 5 1)
'(circular-list 2)
)

(qa
(qa257-cycle 5 2)
'(circular-list 2)
)

(qa
(qa257-cycle 5 3)
'(circular-list 4)
)

(qa
(qa257-cycle 5 4)
'(circular-list 4)
)

(qa
(eq (string-to-vector nil) (vector))
't
)

(qa
(eq (string-to-vector []) [])
't
)

(qa
(eq (vconcat) [])
't
)

(qa
(eq (vconcat nil "" [] (bool-vector)) (vector))
't
)

(qa
(let ((source [1])) (eq (vconcat source) source))
'nil
)

(qa
(vconcat [1] (list 2) "A" (bool-vector t nil))
'[1 2 65 t nil]
)

(qa
(vconcat (make-byte-code 0 (unibyte-string 192 135) [11] 1) [end])
'[0 "\300\207" [11] 1 end]
)


(princ (qa--done)) (terpri)
