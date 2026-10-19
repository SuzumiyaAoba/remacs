;;; -*- lexical-binding: nil; -*-
(defun qa-html-head--parse (text &optional discard-comments)
  (with-temp-buffer
    (insert text)
    (libxml-parse-html-region nil nil nil discard-comments)))

;; Issue #174: preserve an explicit empty head, but suppress an implied one.
;; Expected values freshly verified against GNU Emacs 31.1 with libxml2.
;; Include nil and multiple truthy DISCARD-COMMENTS values.
;; issue
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head></head><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; full
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<html><head></head><body>x</body></html>" discard)
    '(html nil (head nil) (body nil "x"))))
;; uppercase
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<HEAD></HEAD><BODY>x</BODY>" discard)
    '(html nil (head nil) (body nil "x"))))
;; mixed-case
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<HeAd></hEaD><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; tag-whitespace
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head \t\n></head ><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; doctype
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<!DOCTYPE html><head></head><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; leading-whitespace
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse " \t\n<head></head><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; attributes
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head id=\"h\" lang=\"ja\"></head><body>x</body>" discard)
    '(html nil (head ((id . "h") (lang . "ja"))) (body nil "x"))))
;; attribute-quotes
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head data-example=\"<head>\" id=main></head><body>x</body>" discard)
    '(html nil (head ((data-example . "<head>") (id . "main"))) (body nil "x"))))
;; empty-attribute
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head id=\"\"></head><body>x</body>" discard)
    '(html nil (head ((id . ""))) (body nil "x"))))
;; self-closing
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head/><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; implicit-end
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; leading-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<!--before--><head></head><body>x</body>" discard)
    (if discard '(html nil (head nil) (body nil "x"))
      '(top nil (comment nil "before") (html nil (head nil) (body nil "x"))))))
;; root-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<html><!--root--><head></head><body>x</body></html>" discard)
    '(html nil (comment nil "root") (head nil) (body nil "x"))))
;; head-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head><!--inside--></head><body>x</body>" discard)
    '(html nil (head nil (comment nil "inside")) (body nil "x"))))
;; explicit-title
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head><title>x</title></head><body>x</body>" discard)
    '(html nil (head nil (title nil "x")) (body nil "x"))))
;; implicit-title
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<title>x</title><body>x</body>" discard)
    '(html nil (head nil (title nil "x")) (body nil "x"))))
;; body-only
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body>x</body>" discard)
    '(html nil (body nil "x"))))
;; p-only
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<p>x</p>" discard)
    '(html nil (body nil (p nil "x")))))
;; html-body
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<html><body>x</body></html>" discard)
    '(html nil (body nil "x"))))
;; comment-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<!--<head></head>--><body>x</body>" discard)
    (if discard '(html nil (body nil "x"))
      '(top nil (comment nil "<head></head>") (html nil (body nil "x"))))))
;; attr-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body data-example=\"<head></head>\">x</body>" discard)
    '(html nil (body ((data-example . "<head></head>")) "x"))))
;; html-attr-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<html data-example=\"<head>\"><body>x</body></html>" discard)
    '(html ((data-example . "<head>")) (body nil "x"))))
;; escaped-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body>&lt;head&gt;&lt;/head&gt;</body>" discard)
    '(html nil (body nil "<head></head>"))))
;; script-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body><script>\"<head></head>\"</script>x</body>" discard)
    '(html nil (body nil (script nil "\"<head></head>\"") "x"))))
;; style-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body><style>p:after {content: \"<head>\"}</style>x</body>" discard)
    '(html nil (body nil (style nil "p:after {content: \"<head>\"}") "x"))))
;; textarea-decoy
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body><textarea>&lt;head&gt;</textarea>x</body>" discard)
    '(html nil (body nil (textarea nil "<head>") "x"))))
;; heading
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<heading></heading><body>x</body>" discard)
    '(html nil (body nil (heading nil) "x"))))
;; late-head
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<body>x<head></head></body>" discard)
    '(html nil (body nil "x"))))
;; after-p-head
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<p>x</p><head></head>" discard)
    '(html nil (body nil (p nil "x")))))
;; end-tag
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "</head><body>x</body>" discard)
    '(html nil (body nil "x"))))
;; ignored-end-then-head
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "</div><head></head><body>x</body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; explicit-then-late
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "<head></head><body>x<head></head></body>" discard)
    '(html nil (head nil) (body nil "x"))))
;; empty
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse "" discard)
    'nil))
;; space
(dolist (discard '(nil t discard 0))
  (qa (qa-html-head--parse " \t\n" discard)
    'nil))

;; Isolate preservation from existing html5ever/libxml2 differences in
;; implicit bodies and duplicate head elements: the first head must survive.
(dolist (text '("<head></head>" "<html><head></head></html>"
                "<head></head><head></head><body>x</body>"))
  (dolist (discard '(nil t discard 0))
    (qa (assq 'head (cddr (qa-html-head--parse text discard))) '(head nil))))

;; Only the selected region matters, even with a head tag outside it.
(qa (with-temp-buffer
      (insert "<head></head><body>x</body>")
      (libxml-parse-html-region 14 (point-max)))
    '(html nil (body nil "x")))
(qa (with-temp-buffer
      (insert "prefix<head></head><body>x</body>suffix")
      (libxml-parse-html-region 7 (- (point-max) 6)))
    '(html nil (head nil) (body nil "x")))
(qa (with-temp-buffer
      (insert "prefix<head></head><body>x</body>suffix")
      (libxml-parse-html-region (- (point-max) 6) 7 nil t))
    '(html nil (head nil) (body nil "x")))
(qa (with-temp-buffer
      (insert "prefix<head></head><body>x</body>suffix")
      (narrow-to-region 7 (- (point-max) 6))
      (goto-char (point-min))
      (let ((before (buffer-string)) (start (point-min)) (end (point-max)))
        (list (libxml-parse-html-region nil nil nil t)
              (equal before (buffer-string)) (= start (point))
              (= start (point-min)) (= end (point-max)))))
    '((html nil (head nil) (body nil "x")) t t t t))
;; Explicit/implicit state is per parse, including empty documents.
(qa (progn
      (qa-html-head--parse "<head></head><body>x</body>")
      (qa-html-head--parse "<body>x</body>"))
    '(html nil (body nil "x")))
(qa (progn
      (qa-html-head--parse "<body>x</body>")
      (qa-html-head--parse "<head></head><body>x</body>"))
    '(html nil (head nil) (body nil "x")))
(qa (progn
      (qa-html-head--parse "<head></head><body>x</body>" t)
      (qa-html-head--parse ""))
    nil)
(qa (progn
      (qa-html-head--parse "<!--<head>--><body>x</body>")
      (qa-html-head--parse "<head></head><body>x</body>" t))
    '(html nil (head nil) (body nil "x")))

(princ (qa--done))
(terpri)
