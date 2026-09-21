;; JSON primitives coverage: parse/serialize/insert/parse-buffer,
;; object types, array types, null/false objects, error paths.

;; --- parse-string basics -------------------------------------------------
(let ((h (json-parse-string "{\"a\":1,\"b\":[true,false,null],\"c\":\"x\"}")))
  (cl-assert (hash-table-p h))
  (cl-assert (= (gethash "a" h) 1))
  (cl-assert (equal (gethash "b" h) [t :false :null]))
  (cl-assert (equal (gethash "c" h) "x")))

;; alist object type
(let ((a (json-parse-string "{\"k\":2}" :object-type 'alist)))
  (cl-assert (equal a '((k . 2)))))
;; plist object type
(let ((p (json-parse-string "{\"k\":2}" :object-type 'plist)))
  (cl-assert (equal p '(:k 2))))
;; array-type list
(let ((l (json-parse-string "[1,2,3]" :array-type 'list)))
  (cl-assert (equal l '(1 2 3))))
;; array-type array (vector, default)
(let ((v (json-parse-string "[1,2]")))
  (cl-assert (equal v [1 2])))
;; null/false objects
(let ((n (json-parse-string "{\"x\":null,\"y\":false}"
                            :null-object 'NUL :false-object 'FLS)))
  (cl-assert (eq (gethash "x" n) 'NUL))
  (cl-assert (eq (gethash "y" n) 'FLS)))
;; numbers: int, negative, float, exponent
(let ((nums (json-parse-string "[0,-3,2.5,1e3,-1.5e-2]")))
  (cl-assert (equal nums [0 -3 2.5 1000.0 -0.015])))
;; strings: escapes and unicode
(cl-assert (equal (json-parse-string "\"a\\nb\\t\\\"q\\\"\"")
                  "a\nb\t\"q\""))
(cl-assert (equal (json-parse-string "\"\\u0041\\u00e9\"") "Aé"))
;; empty containers
(cl-assert (let ((h (json-parse-string "{}")))
             (and (hash-table-p h) (= (hash-table-count h) 0))))
(cl-assert (equal (json-parse-string "[]") []))
;; top-level scalars
(cl-assert (eq (json-parse-string "null") :null))
(cl-assert (eq (json-parse-string "false") :false))
(cl-assert (eq (json-parse-string "true") t))
(cl-assert (= (json-parse-string "42") 42))
(cl-assert (equal (json-parse-string "\"s\"") "s"))

;; --- parse errors --------------------------------------------------------
(cl-assert (consp (condition-case e (json-parse-string "{bad")
                    (error e))))
(cl-assert (consp (condition-case e (json-parse-string "[1,]")
                    (error e))))
(cl-assert (consp (condition-case e (json-parse-string "")
                    (error e))))
;; bad keyword values
(cl-assert (consp (condition-case e
                      (json-parse-string "{}" :object-type 'vector)
                    (error e))))
(cl-assert (consp (condition-case e
                      (json-parse-string "[]" :array-type 'hash-table)
                    (error e))))

;; --- serialize -----------------------------------------------------------
(cl-assert (equal (json-serialize [1 "a" t]) "[1,\"a\",true]"))
(cl-assert (equal (json-serialize nil) "{}"))
(let* ((h (make-hash-table)))
  (puthash "k" 7 h)
  (cl-assert (equal (json-serialize h) "{\"k\":7}")))
(cl-assert (equal (json-serialize '((a . 1)) :object-type 'alist
                                          :null-object :null)
                  "{\"a\":1}"))
(cl-assert (equal (json-serialize '(:a 1) :object-type 'plist
                                          :null-object :null)
                  "{\"a\":1}"))
;; bare list of ints is treated as alist/plist -> symbolp error (GNU too)
(cl-assert (consp (condition-case e (json-serialize '(1 2))
                    (error e))))
(cl-assert (equal (json-serialize "s") "\"s\""))
(cl-assert (equal (json-serialize 3.5) "3.5"))
;; :null-object required with explicit :object-type on GNU
(cl-assert (consp (condition-case e
                      (json-serialize '((a . 1)) :object-type 'alist)
                    (error e))))
;; escaping in serialization
(cl-assert (equal (json-serialize "a\"b\n") "\"a\\\"b\\n\""))
;; round-trip
(let* ((src "{\"m\":[1,2,{\"n\":null}],\"s\":\"hi\"}")
       (rt (json-serialize (json-parse-string src))))
  (cl-assert (stringp rt)))

;; --- json-insert / json-parse-buffer --------------------------------------
(with-temp-buffer
  (json-insert '((x . 5)))
  (cl-assert (string-match "x" (buffer-string))))
(with-temp-buffer
  (insert "{\"in\":true}")
  (goto-char (point-min))
  (let ((r (json-parse-buffer)))
    (cl-assert (eq (gethash "in" r) t))))
(with-temp-buffer
  (insert "zzz {\"later\":1}")
  (goto-char (point-min))
  (forward-char 4)
  (let ((r (json-parse-buffer)))
    (cl-assert (= (gethash "later" r) 1))))

(princ "json-ok")
