;;; -*- lexical-binding: nil; -*-
;; Issue #168: XML 1.0 literal whitespace and character-reference ordering.
;; Expected values verified with GNU Emacs 31.1 + libxml2.

(defun qa-xml-normalize--parse (text &optional discard-comments)
  (with-temp-buffer
    (insert text)
    (libxml-parse-xml-region nil nil nil discard-comments)))

;; Exact issue reproductions.
(qa (qa-xml-normalize--parse "<a>one\r\ntwo\rthree</a>")
    '(a nil "one\ntwo\nthree"))
(qa (qa-xml-normalize--parse "<a x=\"one\ttwo\nthree\rfour\"/>")
    '(a ((x . "one two three four"))))

;; Raw input, parsed character data, parsed attribute, and literal
;; CDATA/comment content. XML 1.0 leaves NEL, LS and NBSP unchanged.
(dolist (case
         '(("a" "a" "a" "a")
           ("\r" "\n" " " "\n")
           ("\n" "\n" " " "\n")
           ("\r\n" "\n" " " "\n")
           ("\r\r\n" "\n\n" "  " "\n\n")
           ("\n\r" "\n\n" "  " "\n\n")
           ("\t\n\r" "\t\n\n" "   " "\t\n\n")
           (" \t\r\n " " \t\n " "    " " \t\n ")
           ("\r\r" "\n\n" "  " "\n\n")
           ("\n\n" "\n\n" "  " "\n\n")
           ("\t\t" "\t\t" "  " "\t\t")
           ("é雪\r\n末" "é雪\n末" "é雪 末" "é雪\n末")
           ("\u0085\u2028\u00a0" "\u0085\u2028\u00a0" "\u0085\u2028\u00a0" "\u0085\u2028\u00a0")
           ("&amp;&lt;&gt;&quot;&apos;" "&<>\"'" "&<>\"'" "&amp;&lt;&gt;&quot;&apos;")
           ("&#9;&#10;&#13;" "\t\n\r" "\t\n\r" "&#9;&#10;&#13;")
           ("&#x9;&#xA;&#xD;" "\t\n\r" "\t\n\r" "&#x9;&#xA;&#xD;")
           ("&#13;\n" "\r\n" "\r " "&#13;\n")
           ("\r&#10;" "\n\n" " \n" "\n&#10;")
           ("\r&#13;\n" "\n\r\n" " \r " "\n&#13;\n")
           ("&#13;&#10;" "\r\n" "\r\n" "&#13;&#10;")
           ("&#x20;&#32;" "  " "  " "&#x20;&#32;")
           ("&#38;#13;" "&#13;" "&#13;" "&#38;#13;")
           ("&amp;#13;" "&#13;" "&#13;" "&amp;#13;")
           ("&#133;&#8232;&#160;" "\u0085\u2028\u00a0" "\u0085\u2028\u00a0" "&#133;&#8232;&#160;")
           ))
  (let ((raw (nth 0 case))
        (text (concat "A" (nth 1 case) "B"))
        (attr (concat "A" (nth 2 case) "B"))
        (literal (concat "A" (nth 3 case) "B")))
    (qa (qa-xml-normalize--parse (concat "<a>A" raw "B</a>"))
        (list 'a nil text))
    ;; Cover the Empty and Start event attribute paths, both quote styles.
    (qa (qa-xml-normalize--parse (concat "<a x=\"A" raw "B\"/>"))
        (list 'a (list (cons 'x attr))))
    (qa (qa-xml-normalize--parse (concat "<a x='A" raw "B'></a>"))
        (list 'a (list (cons 'x attr))))
    (qa (qa-xml-normalize--parse (concat "<a><![CDATA[A" raw "B]]></a>"))
        (list 'a nil literal))
    (qa (qa-xml-normalize--parse (concat "<a><!--A" raw "B--></a>"))
        (list 'a nil (list 'comment nil literal)))))

;; CDATA remains a separate child; references merge only with ordinary text.
(qa (qa-xml-normalize--parse "<a>left\r<![CDATA[mid\r\n&#13;]]>right&#13;\n</a>")
    '(a nil "left\n" "mid\n&#13;" "right\r\n"))
(qa (qa-xml-normalize--parse "<a>A\r<![CDATA[\nB]]></a>")
    '(a nil "A\n" "\nB"))
(qa (qa-xml-normalize--parse "<a><![CDATA[A\r]]>\nB</a>")
    '(a nil "A\n" "\nB"))
(qa (qa-xml-normalize--parse "<a>\r&amp;\n</a>")
    '(a nil "\n&\n"))
(qa (qa-xml-normalize--parse "<a x=' \t\r\n  '/>")
    '(a ((x . "     "))))
(qa (qa-xml-normalize--parse "<a x='a\r\nb' y='c\rd' z='e\nf'/>")
    '(a ((x . "a b") (y . "c d") (z . "e f"))))
(qa (qa-xml-normalize--parse "<a xml:space='preserve'> \t\r\n </a>")
    '(a ((space . "preserve")) " \t\n "))
(qa (qa-xml-normalize--parse "<!--a\r\nb\rc--><a/><!--d\r-->")
    '(top nil (comment nil "a\nb\nc") (a nil) (comment nil "d\n")))
(qa (qa-xml-normalize--parse "<!--a\r--><a><!--b\r\n--></a>" t)
    '(a nil (comment nil "b\n")))
;; The parser must normalize a copy, never rewrite buffer text or positions.
(qa (with-temp-buffer
      (insert "prefix<a x='one\r\ntwo'>三\rfour</a>suffix")
      (let ((original (buffer-string)))
        (narrow-to-region 7 (- (point-max) 6))
        (goto-char (point-min))
        (let ((dom (libxml-parse-xml-region)))
          (widen)
          (list dom (equal original (buffer-string)) (point)))))
    '((a ((x . "one two")) "三\nfour") t 7))
;; Adjacent HTML uses its own normalization rules: literal TAB/LF in
;; attributes are retained instead of being converted to spaces.
(qa (with-temp-buffer
      (insert "<a x='one\ttwo\nthree'>x y</a>")
      (libxml-parse-html-region))
    '(html nil (body nil (a ((x . "one\ttwo\nthree")) "x y"))))

(princ (qa--done))
(terpri)
