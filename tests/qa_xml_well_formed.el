;;; -*- lexical-binding: nil; -*-
;; Issue #170: reject malformed XML while preserving legal XML 1.0 data.
;; Expected results verified with GNU Emacs 31.1 + libxml2.

(defun qa-xml-wf--parse (text &optional discard-comments)
  (with-temp-buffer
    (insert text)
    (libxml-parse-xml-region nil nil nil discard-comments)))

;; Exact issue reproductions, plus both element-event paths and nested
;; errors. A failed attribute must invalidate the whole document.
(dolist (text '("<a x=\"1\" x=\"2\"/>" "<1bad/>"
                "<a>&#0;</a>" "<a>&#1;</a>" "<a>\1</a>"
                "<a><![CDATA[\1]]></a>" "<a><!--bad--comment--></a>"
                "<a x=\"<\"/>" "<a>]]></a>"
                "<a x='1' x='2'></a>" "<r><a x='1' x='2'/></r>"
                "<r><1bad></1bad></r>" "<a 1bad='x'/>"
                "<a good='x' 1bad='y'></a>" "<a missing/>"
                "<a x=bare/>" "<a x/>" "<a x='1' y/>"
                "<a x='unterminated/>" "<a x='1'y='2'/>"
                "<a x='1'y='2'></a>" "<r><a x='1'y='2'/></r>"
                "<a x='&missing;'/>"
                "<a x='&'/>" "<a x='&amp'/>"
                "<a xmlns='urn:a' xmlns='urn:b'/>"
                "<a xmlns:p='urn:a' xmlns:p='urn:b'></a>"
                "<a xmlnsfoo='one' xmlnsfoo='two'/>"
                "<a xmlns:p='urn:p' p:x='one' p:x='two'/>"
                "<a xmlns='&missing;'/>" "<a xmlns:p='&#1;'></a>"
                "<a xmlns='<'/>" "<a xmlnsfoo='<'/>"
                "<a x='<child/>'></a>" "<a>before]]>after</a>"
                "<a><!--bad---></a>" "<a><!-----></a>"))
  (qa (qa-xml-wf--parse text) nil))

;; Comments still need checking when DISCARD-COMMENTS omits their DOM.
(dolist (text '("<!--bad--comment--><a/>" "<a/><!--bad--comment-->"
                "<!--bad---><a/>" "<a><!--bad--comment--></a>"))
  (qa (qa-xml-wf--parse text) nil)
  (qa (qa-xml-wf--parse text t) nil))

;; Every forbidden C0 character, and the forbidden BMP noncharacters.
;; Check literal data in all relevant locations and both reference forms.
(dolist (code '(0 1 2 3 4 5 6 7 8 11 12 14 15 16 17 18 19 20 21 22
               23 24 25 26 27 28 29 30 31 #xfffe #xffff))
  (let ((raw (string code))
        (decimal (format "&#%d;" code))
        (hex (format "&#x%X;" code)))
    (dolist (value (list raw decimal hex))
      (qa (qa-xml-wf--parse (concat "<a>" value "</a>")) nil)
      (qa (qa-xml-wf--parse (concat "<a x='" value "'/>")) nil)
      (qa (qa-xml-wf--parse (concat "<a x='" value "'></a>")) nil))
    (qa (qa-xml-wf--parse (concat "<a><![CDATA[" raw "]]></a>")) nil)
    (qa (qa-xml-wf--parse (concat "<a><!--" raw "--></a>")) nil)
    (qa (qa-xml-wf--parse (concat "<!--" raw "--><a/>") t) nil)
    (qa (qa-xml-wf--parse (concat "<a><?pi " raw "?></a>")) nil)))

;; Malformed numeric references must fail rather than decode permissively
;; or silently become empty attribute values. Surrogates are not chars.
(dolist (ref '("&#;" "&#x;" "&#X41;" "&#+65;" "&#x+41;"
               "&#-1;" "&# 65;" "&#65 ;" "&#xG;" "&#65"
               "&#55296;" "&#xDFFF;" "&#1114112;"
               "&#999999999999999999999999999999;"))
  (qa (qa-xml-wf--parse (concat "<a>" ref "</a>")) nil)
  (qa (qa-xml-wf--parse (concat "<a x='" ref "'/>")) nil)
  (qa (qa-xml-wf--parse (concat "<a xmlns:p='" ref "'/>")) nil))

;; Names use XML's inclusive Unicode ranges, not ASCII or is_alphabetic.
;; Only acceptance is asserted for Unicode names: GNU exposes their
;; UTF-8 bytes in symbol names, a separate preexisting DOM difference.
(dolist (name '("1bad" "-bad" ".bad" "a!" "a?" "a&b" "a=b"
                "\u00b7bad" "\u0300bad" "\u037ebad" "\u200bbad"
                "\u206fbad" "\u3000bad" "\ufdd0bad" "\U000f0000bad"))
  (qa (qa-xml-wf--parse (concat "<" name "/>")) nil)
  (qa (qa-xml-wf--parse (concat "<" name "></" name ">")) nil)
  (qa (qa-xml-wf--parse (concat "<a " name "='v'/>")) nil))
(dolist (name '("_name" "A-0.name" "é" "雪" "π" "\u00c0"
                "\u02ff" "\u0370" "\u037f" "\u200c" "\u2070"
                "\u2c00" "\u3001" "\uf900" "\ufdf0" "\U00010000"
                "\U000effff" "a\u00b7" "a\u0300" "a\u203f"))
  (qa (not (null (qa-xml-wf--parse (concat "<" name "/>")))) t)
  (qa (not (null (qa-xml-wf--parse (concat "<" name "></" name ">")))) t)
  (qa (not (null (qa-xml-wf--parse (concat "<a " name "='v'/>")))) t))

;; XML's allowed ranges include DEL/C1 controls and supplementary
;; noncharacters. References retain their characters after normalization.
(dolist (code '(9 10 13 32 127 133 159 160 #xd7ff #xe000 #xfdd0 #xfffd
               #x10000 #x1fffe #x10ffff))
  (let ((value (string code)))
    (qa (qa-xml-wf--parse (format "<a>&#%d;</a>" code))
        (list 'a nil value))
    (qa (qa-xml-wf--parse (format "<a x='&#x%X;'/>" code))
        (list 'a (list (cons 'x value))))
    (qa (not (null (qa-xml-wf--parse (concat "<a>" value "</a>")))) t)))

;; Validate raw markup before decoding: these decoded strings are legal.
(qa (qa-xml-wf--parse "<a x='&lt;&#60;&#x3C;&amp;&quot;&apos;'>&lt;&#60;&#x3C;&amp;&quot;&apos;</a>")
    '(a ((x . "<<<&\"'")) "<<<&\"'"))
(qa (qa-xml-wf--parse "<a>]]&gt;]&#93;>]]&#x3E;</a>")
    '(a nil "]]>]]>]]>"))
(qa (qa-xml-wf--parse "<a x=']]>'><![CDATA[<&#0;]]><!--<&#0;--></a>")
    '(a ((x . "]]>")) "<&#0;" (comment nil "<&#0;")))
(qa (qa-xml-wf--parse "<a x='&amp;#0;'>&amp;#0;</a>")
    '(a ((x . "&#0;")) "&#0;"))
(qa (qa-xml-wf--parse "<a><!--one-two--><!----></a>")
    '(a nil (comment nil "one-two") (comment nil "")))
(qa (qa-xml-wf--parse "<a x='1' X='2'><a x='3'/></a>")
    '(a ((x . "1") (X . "2")) (a ((x . "3")))))
;; Local names alone do not identify duplicate attributes. Default
;; namespaces don't apply to attributes; distinct namespace URIs coexist.
(qa (qa-xml-wf--parse "<p:a xmlns='urn:r' xmlns:p='urn:p' xmlns:q='urn:q' x='0' p:x='1' q:x='2'/>")
    '(a ((x . "0") (x . "1") (x . "2"))))
(qa (qa-xml-wf--parse "<a xmlnsfoo='keep' xmlns:p='urn:p'><p:b xmlns:p='urn:inner' p:x='v'/></a>")
    '(a ((xmlnsfoo . "keep")) (b ((x . "v")))))

;; Parsing a bad region doesn't mutate its buffer, point, or narrowing.
(qa (with-temp-buffer
      (insert "prefix<a x='1' x='2'/>suffix")
      (narrow-to-region 7 (- (point-max) 6))
      (goto-char (point-min))
      (let ((before (buffer-string)) (start (point-min)) (end (point-max)))
        (list (libxml-parse-xml-region) (equal before (buffer-string))
              (= start (point)) (= start (point-min)) (= end (point-max)))))
    '(nil t t t t))
;; The lenient HTML parser does not acquire XML's well-formedness checks.
(qa (with-temp-buffer
      (insert "<p x='one' x='two'>]]></p>")
      (libxml-parse-html-region))
    '(html nil (body nil (p ((x . "one")) "]]>"))))

(princ (qa--done))
(terpri)
