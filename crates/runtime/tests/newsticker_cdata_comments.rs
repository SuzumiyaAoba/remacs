use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const CASES: &str = r#"(progn
  (unless (and (featurep 'xml)
               (fboundp 'xml-parse-region)
               (fboundp 'libxml-parse-xml-region)
               (fboundp 'newsticker--do-xml-workarounds)
               (fboundp 'newsticker--remove-xml-comments-preserving-markup))
    (error "Real XML/newsticker source definitions were not loaded"))
  (let* ((cdata-first
          "before <!--literal--> >\n <description><img src='x'/></description> after")
         (cdata-second " and second <!--literal2--> with --> and ]] still text")
         (extra-cdata
          (let ((index 0) result)
            (while (< index 13)
              (push (format " [extra-%d:%s]" index
                            (if (= index 10)
                                (concat "NEWSTICKERXMLTOKEN0-1-X/"
                                        "NEWSTICKERXMLTOKEN0X-1-X/"
                                        "newstickerxmltoken0-1-x/"
                                        "newstickerxmltoken0x-1-x")
                              ""))
                    result)
              (setq index (1+ index)))
            (nreverse result)))
         (extra-sections
          (mapconcat (lambda (text) (concat "<![CDATA[" text "]]>"))
                     extra-cdata ""))
         (cdata-source
          (concat "<![CDATA[" cdata-first "]]>middle<![CDATA[" cdata-second "]]>"
                  extra-sections))
         (marker-text
          "newstickerxmltoken0xx-0-x|NeWsTiCkErXmLtOkEn0Xx-1-X")
         (xml
          (concat "<?xml version='1.0'?>"
                  "<!DOCTYPE rss SYSTEM 'feed<!--doctype-literal-->.dtd'>"
                  "<rss title='a>b' commentish='&lt;!--attribute-literal-->'>"
                  "<channel><description><![CDATA[" cdata-first
                  "]]>middle<![CDATA[" cdata-second "]]>" extra-sections "</description>"
                  "<?reader note='<!--pi-literal-->'?>"
                  "<title>before<!--ordinary-comment-->after</title>"
                  "<marker>" marker-text "</marker>"
                  "</channel></rss>"))
         (expected-text
          (concat cdata-first "middle" cdata-second
                  (mapconcat #'identity extra-cdata ""))))
    ;; Deliberately include text that each older regex workaround could
    ;; mistake for markup while it is inside a CDATA section.
    (with-temp-buffer
      (insert xml)
      (let ((case-fold-search t))
        (newsticker--do-xml-workarounds))
      (let ((contents (buffer-string)))
        (unless (and (string-match-p (regexp-quote cdata-source) contents)
                     (string-match-p
                      (regexp-quote "feed<!--doctype-literal-->.dtd") contents)
                     (string-match-p
                      (regexp-quote "<?reader note='<!--pi-literal-->'?>") contents)
                     (string-match-p
                      (regexp-quote "&lt;!--attribute-literal-->") contents)
                     (string-match-p (regexp-quote marker-text) contents)
                     (not (string-match-p
                           (regexp-quote "<!--ordinary-comment-->") contents)))
          (error "XML workaround changed opaque data or kept a real comment")))
      ;; Exercise the real source XML parser, not a parser stub.
      (let* ((tree (xml-parse-region nil nil nil nil))
             (rss (car tree))
             (channel (car (xml-get-children rss 'channel)))
             (description (car (xml-get-children channel 'description)))
             (title (car (xml-get-children channel 'title)))
             (marker (car (xml-get-children channel 'marker)))
             (actual-text (mapconcat #'identity
                                     (xml-node-children description) "")))
        (unless (and (equal actual-text expected-text)
                     (equal (mapconcat #'identity
                                       (xml-node-children title) "")
                            "beforeafter")
                     (equal (xml-get-attribute rss 'title) "a>b")
                     (equal (xml-get-attribute rss 'commentish)
                            "<!--attribute-literal-->")
                     (equal (mapconcat #'identity
                                       (xml-node-children marker) "")
                            marker-text))
          (error "XML source parser output changed: %S" tree))))
      ;; The libxml route removes comments separately, so cover its safe
      ;; removal helper and the real region parser too.
      (with-temp-buffer
        (insert xml)
        (goto-char (point-min))
        (search-forward "<!--literal-->")
        (let ((begin (match-beginning 0))
              (end (match-end 0)))
          (put-text-property begin end 'remacs-opaque-test 'preserved))
        (let ((case-fold-search t))
          (newsticker--remove-xml-comments-preserving-markup))
        (goto-char (point-min))
        (search-forward "<!--literal-->")
        (unless (eq (get-text-property (match-beginning 0) 'remacs-opaque-test)
                    'preserved)
          (error "CDATA text properties were not restored"))
        (let ((contents (buffer-string)))
          (unless (and (string-match-p (regexp-quote cdata-source) contents)
                       (string-match-p
                        (regexp-quote "feed<!--doctype-literal-->.dtd")
                        contents)
                       (string-match-p
                        (regexp-quote "<?reader note='<!--pi-literal-->'?>")
                        contents)
                       (string-match-p (regexp-quote marker-text) contents)
                       (not (string-match-p
                             (regexp-quote "<!--ordinary-comment-->")
                             contents)))
            (error "libxml comment removal changed opaque XML data")))
        (let* ((rss (libxml-parse-xml-region (point-min) (point-max) nil))
               (channel (car (xml-get-children rss 'channel)))
               (description (car (xml-get-children channel 'description)))
               (title (car (xml-get-children channel 'title)))
               (marker (car (xml-get-children channel 'marker)))
               (actual-text (mapconcat #'identity
                                       (xml-node-children description) "")))
          (unless (and (equal actual-text expected-text)
                       (equal (mapconcat #'identity
                                       (xml-node-children title) "")
                            "beforeafter")
                       (equal (xml-get-attribute rss 'title) "a>b")
                       (equal (xml-get-attribute rss 'commentish)
                              "<!--attribute-literal-->")
                       (equal (mapconcat #'identity
                                         (xml-node-children marker) "")
                              marker-text))
            (error "libxml parser output changed: %S" rss))))
    ;; Unterminated CDATA must signal instead of making the scanner loop.
    (with-temp-buffer
      (insert "<root><![CDATA[unfinished")
      (unless (condition-case nil
                  (progn (newsticker--xml-protect-markup-data) nil)
                (error t))
        (error "Unterminated CDATA was not rejected")))
    ;; A later lexer error must roll back placeholders inserted for earlier
    ;; complete opaque regions in both parsing paths.
    (dolist (case '((newsticker--do-xml-workarounds
                     "<root><![CDATA[keep this]]><!--unfinished")
                    (newsticker--remove-xml-comments-preserving-markup
                     "<root><![CDATA[keep this]]><?unfinished")))
      (with-temp-buffer
        (insert (cadr case))
        (let ((before (buffer-string)))
          (unless (condition-case nil
                      (progn (funcall (car case)) nil)
                    (error t))
            (error "Malformed XML was not rejected: %S" (car case)))
          (unless (equal (buffer-string) before)
            (error "Failed XML cleanup left placeholders: %S" (car case))))))
  t))"#;

#[test]
fn newsticker_comment_removal_preserves_cdata_in_both_xml_paths_2157() {
    let root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let xml_source = root.join("lisp/xml.el").canonicalize().unwrap();
    let backend_source = root.join("lisp/newst-backend.el").canonicalize().unwrap();
    let code = format!(
        "(progn (load {} nil t t) (load {} nil t t) {})",
        lisp_string(xml_source.to_str().unwrap()),
        lisp_string(backend_source.to_str().unwrap()),
        CASES
    );
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => {
            let symbol = format!("{}", interp.prin1_to_string(&signal.symbol()));
            let data = format!("{}", interp.prin1_to_string(&signal.data()));
            panic!("{symbol} {data}")
        }
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
