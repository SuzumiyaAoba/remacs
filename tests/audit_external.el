;;; audit_external.el --- external parser/database differential probes -*- lexical-binding: t; -*-
;; Run the SAME file with GNU Emacs 31.1 and current remacs, using -Q --batch -l.
;; Databases are in-memory.  XML cases contain no external entity URLs.

(defun audit-external-run (id form)
  (let ((print-circle t) (print-length 50) (print-level 20)
        (print-escape-newlines t) (print-escape-control-characters t))
    (when (getenv "REMACS_AUDIT_FORMS")
      (princ "FORM|") (prin1 id) (princ "|") (prin1 form) (terpri))
    (princ "AUDIT|")
    (prin1 id)
    (princ "|")
    (prin1 (condition-case e (list 'ok (eval form t))
             (error (list 'error e)))))
  (terpri))

(audit-external-run "versions"
 '(list emacs-version (sqlite-available-p) (libxml-available-p)))

(dolist (input '("<a/>" "<a>one\r\ntwo\rthree</a>"
                 "<a x=\"one\ttwo\nthree\rfour\"/>"
                 "<a xmlnsfoo=\"keep\"/>"
                 "<a x=\"1\" x=\"2\"/>"
                 "<a x=\"&missing;\"/>" "<a>&missing;</a>"
                 "<a>&#0;</a>" "<a>&#1;</a>"
                 "<a>\001</a>" "<a><![CDATA[\001]]></a>"
                 "<!DOCTYPE a [<!ENTITY word 'hello'>]><a>&word;</a>"
                 "<!DOCTYPE a [<!ENTITY word 'hello'>]><a x=\"&word;\"/>"
                 "<a><!--bad--comment--></a>" "<1bad/>"
                 "<a x=\"<\"/>" "<a>]]></a>"))
  (audit-external-run (concat "xml:" input)
    `(with-temp-buffer (insert ,input) (libxml-parse-xml-region))))

(dolist (input '("<p>ok</p>" "<!--before--><p><!--inside-->text</p><!--after-->"
                 "<head></head><body>x</body>"
                 "<p>one</p><p>two</p>"
                 "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>"
                 "<table>text<tr><td>a</td></tr></table>"
                 "<p><div>x</div>y</p>" "<svg viewBox=\"0 0 1 1\"><circle/></svg>"))
  (audit-external-run (concat "html:" input)
    `(with-temp-buffer (insert ,input) (libxml-parse-html-region)))
  (audit-external-run (concat "html-discard:" input)
    `(with-temp-buffer (insert ,input) (libxml-parse-html-region nil nil nil t))))

(dolist (entry
 '(("sql:missing-all" (sqlite-select db "select ?, ?"))
   ("sql:missing-one" (sqlite-select db "select ?, ?" '(7)))
   ("sql:extra-one" (sqlite-select db "select ?" '(7 8)))
   ("sql:nil-bind" (sqlite-select db "select ?" '(nil)))
   ("sql:true-bind" (sqlite-select db "select ?" '(t)))
   ("sql:blob-bind" (let ((s (caar (sqlite-select db "select ?" (list (unibyte-string 0 128 255))))))
                      (list (multibyte-string-p s) (string-to-list s))))
   ("sql:blob-read" (let ((s (caar (sqlite-select db "select x'0080ff'"))))
                      (list (multibyte-string-p s) (string-to-list s))))
   ("sql:bigint-bind" (sqlite-select db "select ?" '(9223372036854775808)))
   ("sql:empty" (sqlite-execute db ""))
   ("sql:empty-select" (sqlite-select db ""))
   ("sql:bad-return-type" (sqlite-select db "select 1" nil 'bogus))
   ("sql:lazy-insert"
    (progn (sqlite-execute db "create table t(a)")
           (sqlite-select db "insert into t values(7) returning a" nil 'set)
           (sqlite-select db "select * from t")))
   ("sql:close-active"
    (let ((s (sqlite-select db "select 1 union all select 2" nil 'set)))
      (list (sqlite-close db) (sqlite-next s) (sqlite-next s))))
   ("sql:forged" (let ((fake (record 'sqlite 'db 999999)))
                    (list (sqlitep fake) (sqlite-close fake))))
   ("sql:select-error" (sqlite-select db "select * from nonexistent"))
   ("sql:constraint-error" (progn (sqlite-execute db "create table t(x unique)")
                                 (sqlite-execute db "insert into t values(1)")
                                 (sqlite-execute db "insert into t values(1)")))))
  (audit-external-run (car entry)
    `(let ((db (sqlite-open)))
       (unwind-protect ,(cadr entry) (sqlite-close db)))))

(princ "AUDIT-DONE\n")
