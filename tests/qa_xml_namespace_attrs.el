;;; -*- lexical-binding: nil; -*-
;; Issue #169: only xmlns and xmlns:prefix are namespace declarations.
;; Expected values verified with GNU Emacs 31.1 + libxml2.

(defun qa-xml-ns--parse (text)
  (with-temp-buffer
    (insert text)
    (libxml-parse-xml-region)))

;; Exact issue reproduction.
(qa (qa-xml-ns--parse "<a xmlnsfoo=\"keep\"/>")
    '(a ((xmlnsfoo . "keep"))))

;; Full qualified names, not a loose xmlns prefix or local name, decide
;; which attributes to omit. Cover Empty/Start, both quotes, and nesting.
(dolist (name '("xmlnsfoo" "xmlnsFoo" "xmlns_" "xmlns-foo" "xmlns.foo"
                "xmlns0" "xmlnsxmlns" "xmln"
                "xml" "XMLNS" "Xmlns" "xXmlns" "other"))
  (let ((attr (list (cons (intern name) "keep"))))
    (qa (qa-xml-ns--parse (concat "<a " name "=\"keep\"/>"))
        (list 'a attr))
    (qa (qa-xml-ns--parse (concat "<a " name "='keep'></a>"))
        (list 'a attr))
    (qa (qa-xml-ns--parse
         (concat "<r xmlns='urn:root'><a " name "='keep'/></r>"))
        (list 'r nil (list 'a attr)))
    (qa (qa-xml-ns--parse
         (concat "<p:a xmlns:p='urn:test' p:" name "='keep'></p:a>"))
        (list 'a attr))))

;; Genuine declarations disappear regardless of attribute position.
(qa (qa-xml-ns--parse "<a xmlns='urn:default'/>") '(a nil))
(qa (qa-xml-ns--parse "<p:a xmlns:p='urn:prefix'></p:a>") '(a nil))
(qa (qa-xml-ns--parse "<a xmlns=''/>") '(a nil))
(qa (qa-xml-ns--parse
     "<a xmlnsfoo='one' xmlns='urn:default' xmlnsbar='two' xmlns:p='urn:p' p:x='three'/>")
    '(a ((xmlnsfoo . "one") (xmlnsbar . "two") (x . "three"))))
(qa (qa-xml-ns--parse
     "<a xmlns:p='urn:p' p:xmlns='ordinary' p:xmlnsfoo='keep'></a>")
    '(a ((xmlns . "ordinary") (xmlnsfoo . "keep"))))
(qa (qa-xml-ns--parse
     "<xmlnsfoo:a xmlns:xmlnsfoo='urn:p' xmlnsfoo:x='keep' xmlnsfoo='ordinary'/>")
    '(a ((x . "keep") (xmlnsfoo . "ordinary"))))
(qa (qa-xml-ns--parse
     "<r xmlns='urn:r' xmlnsfoo='outer'><a xmlns='' xmlnsfoo='inner'></a></r>")
    '(r ((xmlnsfoo . "outer")) (a ((xmlnsfoo . "inner")))))
(qa (qa-xml-ns--parse
     "<r xmlns:p='urn:outer'><p:a xmlns:p='urn:inner' xmlnsfoo='keep'/></r>")
    '(r nil (a ((xmlnsfoo . "keep")))))
(qa (qa-xml-ns--parse
     "<a xmlnsfoo='' xmlnsbar='one&amp;two' xmlnsbaz='雪&#13;\r\nend'/>")
    '(a ((xmlnsfoo . "") (xmlnsbar . "one&two") (xmlnsbaz . "雪\r end"))))
(qa (qa-xml-ns--parse
     "<a xmlnsfoo='keep'>xmlns:text<!--xmlns:comment--><![CDATA[xmlns:data]]></a>")
    '(a ((xmlnsfoo . "keep")) "xmlns:text" (comment nil "xmlns:comment") "xmlns:data"))

;; Region parsing must keep the buffer and point intact.
(qa (with-temp-buffer
      (insert "prefix<a xmlnsfoo='keep'/>suffix")
      (let ((original (buffer-string)))
        (narrow-to-region 7 (- (point-max) 6))
        (goto-char (point-min))
        (let ((dom (libxml-parse-xml-region)))
          (widen)
          (list dom (equal original (buffer-string)) (point)))))
    '((a ((xmlnsfoo . "keep"))) t 7))
;; HTML attributes have their own namespace rules.
(qa (with-temp-buffer
      (insert "<p xmlnsfoo='keep' xmlns='urn:html'>text</p>")
      (libxml-parse-html-region))
    '(html nil (body nil (p ((xmlnsfoo . "keep") (xmlns . "urn:html")) "text"))))

(princ (qa--done))
(terpri)
