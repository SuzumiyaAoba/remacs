;;; -*- lexical-binding: nil; -*-
(defun qa-html-tbody--parse (text &optional discard-comments)
  (with-temp-buffer
    (insert text)
    (libxml-parse-html-region nil nil nil discard-comments)))

;; Issue #175: explicit tbody provenance belongs to each element, including
;; distinct groups in the same table and tables nested inside cells.
;; Expected DOMs freshly verified with GNU Emacs 31.1 and libxml2.
;; Truthy DISCARD-COMMENTS values must not change inner table structure.
;; issue
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; reverse
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>b</td></tr></tbody></table><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "b")))) (table nil (tr nil (td nil "a")))))))
;; implicit-only
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")))))))
;; explicit-only
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "b"))))))))
;; both-implicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tr><td>b</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tr nil (td nil "b")))))))
;; both-explicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>a</td></tr></tbody></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "a")))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; three-tables
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table><table><tr><td>c</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b")))) (table nil (tr nil (td nil "c")))))))
;; implicit-between
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>b</td></tr></tbody></table><table><tr><td>a</td></tr></table><table><tbody><tr><td>c</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "b")))) (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "c"))))))))
;; same-table-implicit-first
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) (tbody nil (tr nil (td nil "b"))))))))
;; same-table-explicit-first
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>a</td></tr></tbody><tr><td>b</td></tr></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "a"))) (tr nil (td nil "b")))))))
;; same-table-implicit-explicit-implicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr><tbody><tr><td>b</td></tr></tbody><tr><td>c</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) (tbody nil (tr nil (td nil "b"))) (tr nil (td nil "c")))))))
;; same-table-explicit-implicit-explicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>a</td></tr></tbody><tr><td>b</td></tr><tbody><tr><td>c</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "a"))) (tr nil (td nil "b")) (tbody nil (tr nil (td nil "c"))))))))
;; multiple-groups
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>a</td></tr></tbody><tbody><tr><td>b</td></tr></tbody><tr><td>c</td></tr><tr><td>d</td></tr></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "a"))) (tbody nil (tr nil (td nil "b"))) (tr nil (td nil "c")) (tr nil (td nil "d")))))))
;; nested-explicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>outer<table><tbody><tr><td>inner</td></tr></tbody></table></td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "outer" (table nil (tbody nil (tr nil (td nil "inner")))))))))))
;; nested-implicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>outer<table><tr><td>inner</td></tr></table></td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "outer" (table nil (tr nil (td nil "inner")))))))))))
;; nested-mixed
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td><table><tbody><tr><td>in-a</td></tr></tbody><tr><td>in-b</td></tr></table></td></tr><tbody><tr><td>out-b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil (table nil (tbody nil (tr nil (td nil "in-a"))) (tr nil (td nil "in-b"))))) (tbody nil (tr nil (td nil "out-b"))))))))
;; three-level-nested
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td><table><tbody><tr><td><table><tr><td>x</td></tr></table></td></tr></tbody></table></td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil (table nil (tbody nil (tr nil (td nil (table nil (tr nil (td nil "x"))))))))))))))
;; uppercase
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><TABLE><TBODY><TR><TD>b</TD></TR></TBODY></TABLE>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; mixed-case
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tBoDy><tr><td>b</td></tr></TbOdY></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; tag-whitespace
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody \t\n><tr><td>b</td></tr></tbody ></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; attributed
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table id=\"second\"><tbody id=\"body\" class=\"c\"><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table ((id . "second")) (tbody ((id . "body") (class . "c")) (tr nil (td nil "b"))))))))
;; quoted-angle
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody data-example=\"<tbody>\"><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody ((data-example . "<tbody>")) (tr nil (td nil "b"))))))))
;; empty-explicit
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil))))))
;; empty-before-row
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody></tbody><tr><td>b</td></tr></table>" discard)
    '(html nil (body nil (table nil (tbody nil) (tr nil (td nil "b")))))))
;; empty-between-rows
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr><tbody></tbody><tr><td>b</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) (tbody nil) (tr nil (td nil "b")))))))
;; omitted-tbody-end
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; omitted-row-cell-ends
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</table><table><tbody><tr><td>b</table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; unclosed-implicit-before-group
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a<tbody><tr><td>b</table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) (tbody nil (tr nil (td nil "b"))))))))
;; unclosed-explicit-before-group
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tbody><tr><td>a<tbody><tr><td>b</table>" discard)
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "a"))) (tbody nil (tr nil (td nil "b"))))))))
;; thead-and-tfoot
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><thead><tr><td>h</td></tr></thead><tr><td>a</td></tr><tbody><tr><td>b</td></tr></tbody><tfoot><tr><td>f</td></tr></tfoot></table>" discard)
    '(html nil (body nil (table nil (thead nil (tr nil (td nil "h"))) (tr nil (td nil "a")) (tbody nil (tr nil (td nil "b"))) (tfoot nil (tr nil (td nil "f"))))))))
;; explicit-colgroup
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><colgroup><col></colgroup><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (colgroup nil (col nil)) (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; caption
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><caption>title</caption><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (caption nil "title") (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; leading-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<!--before--><table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    (if discard '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))
      '(top nil (comment nil "before") (html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))))
;; table-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><!--table--><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (comment nil "table") (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; group-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr></table><table><tbody><!--group--><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (comment nil "group") (tr nil (td nil "b"))))))))
;; between-row-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td>a</td></tr><!--between--><tbody><tr><td>b</td></tr></tbody><tr><td>c</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) (comment nil "between") (tbody nil (tr nil (td nil "b"))) (tr nil (td nil "c")))))))
;; comment-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<!--<tbody>--><table><tr><td>a</td></tr></table>" discard)
    (if discard '(html nil (body nil (table nil (tr nil (td nil "a")))))
      '(top nil (comment nil "<tbody>") (html nil (body nil (table nil (tr nil (td nil "a")))))))))
;; table-comment-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><!--<tbody>--><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table nil (comment nil "<tbody>") (tr nil (td nil "a")))))))
;; cell-comment-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table><tr><td><!--<tbody>-->a</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil (comment nil "<tbody>") "a")))))))
;; double-quote-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<div data-example=\"<tbody>\"></div><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (div ((data-example . "<tbody>"))) (table nil (tr nil (td nil "a")))))))
;; single-quote-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<div data-example='<tbody>'></div><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (div ((data-example . "<tbody>"))) (table nil (tr nil (td nil "a")))))))
;; table-attribute-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<table title=\"<tbody>\"><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table ((title . "<tbody>")) (tr nil (td nil "a")))))))
;; escaped-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<p>&lt;tbody&gt;</p><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (p nil "<tbody>") (table nil (tr nil (td nil "a")))))))
;; script-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<body><script>\"<tbody>\"</script><table><tr><td>a</td></tr></table></body>" discard)
    '(html nil (body nil (script nil "\"<tbody>\"") (table nil (tr nil (td nil "a")))))))
;; style-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<body><style>p:after {content:\"<tbody>\"}</style><table><tr><td>a</td></tr></table></body>" discard)
    '(html nil (body nil (style nil "p:after {content:\"<tbody>\"}") (table nil (tr nil (td nil "a")))))))
;; textarea-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<textarea>&lt;tbody&gt;</textarea><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (textarea nil "<tbody>") (table nil (tr nil (td nil "a")))))))
;; title-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<title>&lt;tbody&gt;</title><table><tr><td>a</td></tr></table>" discard)
    '(html nil (head nil (title nil "<tbody>")) (body nil (table nil (tr nil (td nil "a")))))))
;; prefix-tag-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<tbody-other></tbody-other><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (tbody-other nil) (table nil (tr nil (td nil "a")))))))
;; end-tag-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "</tbody><table><tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")))))))
;; explicit-head
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<head></head><body><table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table></body>" discard)
    '(html nil (head nil) (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; doctype
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<!doctype html><table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b"))))))))
;; no-table
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "<p>text</p>" discard)
    '(html nil (body nil (p nil "text")))))
;; empty
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse "" discard)
    'nil))
;; space
(dolist (discard '(nil t discard 0))
  (qa (qa-html-tbody--parse " \t\n" discard)
    'nil))

;; html5ever and libxml2 already recover self-closing tbody and orphan td
;; differently. Pin the per-node provenance independently of those shapes.
(dolist (discard '(nil t discard 0))
  (dolist (text '("<table><tr><td>a</td></tr></table><table><tbody/><tr><td>b</td></tr></table>"
                  "<table><td>a</td></table><table><tbody><tr><td>b</td></tr></tbody></table>"))
    (let ((tables (cddr (assq 'body (cddr (qa-html-tbody--parse text discard))))))
      (qa (assq 'tbody (cddr (car tables))) nil)
      (qa (car (assq 'tbody (cddr (cadr tables)))) 'tbody))))

;; Provenance is region-local, including reversed and narrowed regions.
(dolist (discard '(nil t))
  (qa (with-temp-buffer
        (insert "<table><tbody></tbody></table><table><tr><td>a</td></tr></table>")
        (libxml-parse-html-region 31 (point-max) nil discard))
      '(html nil (body nil (table nil (tr nil (td nil "a"))))))
  (qa (with-temp-buffer
        (insert "prefix<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>suffix")
        (libxml-parse-html-region (- (point-max) 6) 7 nil discard))
      '(html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b")))))))
  (qa (with-temp-buffer
        (insert "prefix<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>suffix")
        (narrow-to-region 7 (- (point-max) 6))
        (goto-char (point-min))
        (let ((before (buffer-string)) (start (point-min)) (end (point-max)))
          (list (libxml-parse-html-region nil nil nil discard)
                (equal before (buffer-string)) (= start (point))
                (= start (point-min)) (= end (point-max)))))
      '((html nil (body nil (table nil (tr nil (td nil "a"))) (table nil (tbody nil (tr nil (td nil "b")))))) t t t t)))

;; State must not leak across parses, including an intervening empty input.
(qa (progn
      (qa-html-tbody--parse "<table><tbody><tr><td>b</td></tr></tbody></table>")
      (qa-html-tbody--parse "<table><tr><td>a</td></tr></table>"))
    '(html nil (body nil (table nil (tr nil (td nil "a"))))))
(qa (progn
      (qa-html-tbody--parse "<table><tr><td>a</td></tr></table>")
      (qa-html-tbody--parse "<table><tbody><tr><td>b</td></tr></tbody></table>"))
    '(html nil (body nil (table nil (tbody nil (tr nil (td nil "b")))))))
(qa (progn
      (qa-html-tbody--parse "<table><tbody></tbody></table>")
      (qa-html-tbody--parse ""))
    nil)
(qa (progn
      (qa-html-tbody--parse "<!--<tbody>--><p>x</p>")
      (qa-html-tbody--parse "<table><tr><td>a</td></tr></table>"))
    '(html nil (body nil (table nil (tr nil (td nil "a"))))))

(princ (qa--done))
(terpri)
