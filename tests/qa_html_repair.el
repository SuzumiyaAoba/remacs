;;; -*- lexical-binding: nil; -*-
;; Issue #176: libxml-compatible table character data, ignored unmatched
;; paragraph end tags, and ASCII-lowercase HTML foreign attribute names.
;; Expected DOMs freshly verified with GNU Emacs 31.1 and libxml2.
;; This does not cover unrelated HTML5/libxml recovery differences such as
;; formatting reconstruction, leading-text wrappers, or foreign tag casing.
(defun qa-html-repair--parse (text &optional discard-comments)
  (with-temp-buffer
    (insert text)
    (libxml-parse-html-region nil nil nil discard-comments)))

;; issue-table
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>text<tr><td>a</td></tr></table>" discard)
    '(html nil (body nil (table nil "text" (tr nil (td nil "a")))))))

;; table-tail
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tr><td>a</td></tr>tail</table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "a")) "tail")))))

;; table-both
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>before<tr><td>a</td></tr>after</table>" discard)
    '(html nil (body nil (table nil "before" (tr nil (td nil "a")) "after")))))

;; table-row
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tr>before<td>a</td>after</tr></table>" discard)
    '(html nil (body nil (table nil (tr nil "before" (td nil "a") "after"))))))

;; table-group
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tbody>before<tr><td>a</td></tr>after</tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil "before" (tr nil (td nil "a")) "after"))))))

;; table-thead
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><thead>before<tr><th>a</th></tr>after</thead></table>" discard)
    '(html nil (body nil (table nil (thead nil "before" (tr nil (th nil "a")) "after"))))))

;; table-tfoot
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tfoot>before<tr><td>a</td></tr>after</tfoot></table>" discard)
    '(html nil (body nil (table nil (tfoot nil "before" (tr nil (td nil "a")) "after"))))))

;; table-groups
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<tbody>B<tr>C<td>D</td>E</tr>F</tbody>G<tr>H<td>I</td>J</tr>K</table>" discard)
    '(html nil (body nil (table nil "A" (tbody nil "B" (tr nil "C" (td nil "D") "E") "F") "G" (tr nil "H" (td nil "I") "J") "K")))))

;; table-nested
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<tr><td>B<table>C<tr>D<td>E</td>F</tr>G</table>H</td></tr>I</table>" discard)
    '(html nil (body nil (table nil "A" (tr nil (td nil "B" (table nil "C" (tr nil "D" (td nil "E") "F") "G") "H")) "I")))))

;; table-two
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<tr><td>B</td></tr>C</table><table>D<tbody>E<tr><td>F</td></tr>G</tbody>H</table>" discard)
    '(html nil (body nil (table nil "A" (tr nil (td nil "B")) "C") (table nil "D" (tbody nil "E" (tr nil (td nil "F")) "G") "H")))))

;; table-entities
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A&amp;B&#x20;C<tr><td>a</td></tr>&#65;&nbsp;</table>" discard)
    '(html nil (body nil (table nil "A&B C" (tr nil (td nil "a")) "A ")))))

;; table-unicode
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>日本語<tr><td>é</td></tr>終</table>" discard)
    '(html nil (body nil (table nil "日本語" (tr nil (td nil "é")) "終")))))

;; table-comments
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<!--x-->B<tr><td>C</td></tr><!--y-->D</table>" discard)
    '(html nil (body nil (table nil "A" (comment nil "x") "B" (tr nil (td nil "C")) (comment nil "y") "D")))))

;; table-comment-modes
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<!--before--><table>A<!--inside--><tr><td>B</td></tr>C</table><!--after-->" discard)
    (if discard '(html nil (body nil (table nil "A" (comment nil "inside") (tr nil (td nil "B")) "C") (comment nil "after"))) '(top nil (comment nil "before") (html nil (body nil (table nil "A" (comment nil "inside") (tr nil (td nil "B")) "C") (comment nil "after")))))))

;; table-mixed-space
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table> \n A \n<tr><td>a</td></tr> \n B \n</table>" discard)
    '(html nil (body nil (table nil " \n A \n" (tr nil (td nil "a")) " \n B \n")))))

;; table-rawscript
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><script>if(a < b) text = \"<p></p>\";</script>A<tr><td>B</td></tr></table>" discard)
    '(html nil (body nil (table nil (script nil "if(a < b) text = \"<p></p>\";") "A" (tr nil (td nil "B")))))))

;; table-caption
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<caption>B</caption>C<tr><td>D</td></tr>E</table>" discard)
    '(html nil (body nil (table nil "A" (caption nil "B") "C" (tr nil (td nil "D")) "E")))))

;; table-formatting
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<b>B</b>C<tr><td>D</td></tr>E</table>" discard)
    '(html nil (body nil (table nil "A" (b nil "B") "C" (tr nil (td nil "D")) "E")))))

;; table-formatting-order
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<b>B</b>C</table>" discard)
    '(html nil (body nil (table nil "A" (b nil "B") "C")))))

;; table-div-order
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<div>B</div>C</table>" discard)
    '(html nil (body nil (table nil "A" (div nil "B") "C")))))

;; table-p-order
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<p>B</p>C</table>" discard)
    '(html nil (body nil (table nil "A" (p nil "B") "C")))))

;; table-row-formatting
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tr>A<b>B</b>C<td>D</td>E</tr></table>" discard)
    '(html nil (body nil (table nil (tr nil "A" (b nil "B") "C" (td nil "D") "E"))))))

;; table-group-formatting
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tbody>A<div>B</div>C<tr><td>D</td></tr>E</tbody></table>" discard)
    '(html nil (body nil (table nil (tbody nil "A" (div nil "B") "C" (tr nil (td nil "D")) "E"))))))

;; table-span-order
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<span>B</span>C</table>" discard)
    '(html nil (body nil (table nil "A" (span nil "B") "C")))))

;; table-div
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<div>B</div>C<tr><td>D</td></tr>E</table>" discard)
    '(html nil (body nil (table nil "A" (div nil "B") "C" (tr nil (td nil "D")) "E")))))

;; table-open
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<tr>B<td>C" discard)
    '(html nil (body nil (table nil "A" (tr nil "B" (td nil "C")))))))

;; table-implicit-groups
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A<tr><td>B</td></tr>C<tbody>D<tr><td>E</td></tr>F</tbody>G<tr><td>H</td></tr>I</table>" discard)
    '(html nil (body nil (table nil "A" (tr nil (td nil "B")) "C" (tbody nil "D" (tr nil (td nil "E")) "F") "G" (tr nil (td nil "H")) "I")))))

;; issue-p
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p><div>x</div>y</p>" discard)
    '(html nil (body nil (p nil) (div nil "x") "y"))))

;; p-tail
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<div>B</div>C</p>D" discard)
    '(html nil (body nil (p nil "A") (div nil "B") "CD"))))

;; p-repeated
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<div>B</div>C</p>D</p>E" discard)
    '(html nil (body nil (p nil "A") (div nil "B") "CDE"))))

;; p-explicit-empty
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p></p><p>A</p><p></p>" discard)
    '(html nil (body nil (p nil) (p nil "A") (p nil)))))

;; p-stray-after-block
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<div>A</div></p>B" discard)
    '(html nil (body nil (div nil "A") "B"))))

;; p-stray-before-block
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "</p><div>A</div>" discard)
    '(html nil (body nil (div nil "A")))))

;; p-closed-twice
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A</p>B</p>C" discard)
    '(html nil (body nil (p nil "A") "BC"))))

;; p-nested-blocks
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<div>B<div>C</div>D</div>E</p>F" discard)
    '(html nil (body nil (p nil "A") (div nil "B" (div nil "C") "D") "EF"))))

;; p-comments
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<div>B</div>C<!--x--></p>D" discard)
    '(html nil (body nil (p nil "A") (div nil "B") "C" (comment nil "x") "D"))))

;; p-decoy-comment
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<div><!--</p>--><p></p></div>" discard)
    '(html nil (body nil (div nil (comment nil "</p>") (p nil))))))

;; p-decoy-attribute
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<div data-example=\"</p>\"><p></p></div>" discard)
    '(html nil (body nil (div ((data-example . "</p>")) (p nil))))))

;; p-script
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<script>var x=\"</p>\";</script><p></p>" discard)
    '(html nil (head nil (script nil "var x=\"</p>\";")) (body nil (p nil)))))

;; p-table
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table>A</p>B<tr><td>C</td></tr>D</p>E</table>" discard)
    '(html nil (body nil (table nil "AB" (tr nil (td nil "C")) "DE")))))

;; p-cell
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tr><td><p>A<div>B</div>C</p>D</td></tr></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil (p nil "A") (div nil "B") "CD")))))))

;; p-list
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<ul><li><p>A<div>B</div>C</p>D</li></ul>" discard)
    '(html nil (body nil (ul nil (li nil (p nil "A") (div nil "B") "CD"))))))

;; p-inline
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<span>B</span>C</p>D" discard)
    '(html nil (body nil (p nil "A" (span nil "B") "C") "D"))))

;; p-upper
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<P>A<DIV>B</DIV>C</P>D" discard)
    '(html nil (body nil (p nil "A") (div nil "B") "CD"))))

;; p-attributes
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p id=\"first\">A<div>B</div>C</p><p id=\"second\"></p>" discard)
    '(html nil (body nil (p ((id . "first")) "A") (div nil "B") "C" (p ((id . "second")))))))

;; issue-svg
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<svg viewBox=\"0 0 1 1\"><circle/></svg>" discard)
    '(html nil (body nil (svg ((viewbox . "0 0 1 1")) (circle nil))))))

;; svg-case
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<SVG VIEWBOX=\"0 0 1 1\" preserveAspectRatio=\"xMidYMid\"><circle pathLength=\"3\"/></SVG>" discard)
    '(html nil (body nil (svg ((viewbox . "0 0 1 1") (preserveaspectratio . "xMidYMid")) (circle ((pathlength . "3"))))))))

;; svg-attrs
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<svg viewBox=\"x\" baseProfile=\"tiny\" contentScriptType=\"x\" zoomAndPan=\"magnify\"></svg>" discard)
    '(html nil (body nil (svg ((viewbox . "x") (baseprofile . "tiny") (contentscripttype . "x") (zoomandpan . "magnify")))))))

;; svg-duplicates
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<svg viewBox=\"first\" viewbox=\"second\" VIEWBOX=\"third\"></svg>" discard)
    '(html nil (body nil (svg ((viewbox . "first")))))))

;; svg-custom
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<svg data-Foo=\"x\" ATTR=\"MixedCaseValue\" aria-LABEL=\"Name\"></svg>" discard)
    '(html nil (body nil (svg ((data-foo . "x") (attr . "MixedCaseValue") (aria-label . "Name")))))))

;; math-attrs
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<math definitionURL=\"x\"><mi mathvariant=\"bold\">x</mi></math>" discard)
    '(html nil (body nil (math ((definitionurl . "x")) (mi ((mathvariant . "bold")) "x"))))))

;; xml-like-html
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<r viewBox=\"X\" definitionURL=\"Y\"/>" discard)
    '(html nil (body nil (r ((viewbox . "X") (definitionurl . "Y")))))))

;; head-control
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<html><head></head><body><p></p></body></html>" discard)
    '(html nil (head nil) (body nil (p nil)))))

;; tbody-control
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<table><tr><td>A</td></tr><tbody><tr><td>B</td></tr></tbody></table>" discard)
    '(html nil (body nil (table nil (tr nil (td nil "A")) (tbody nil (tr nil (td nil "B"))))))))

;; combined
(dolist (discard '(nil t discard 0))
  (qa (qa-html-repair--parse "<p>A<div><svg viewBox=\"0 0 1 1\"><circle/></svg></div>B</p>C<table>D<tr><td>E</td></tr>F</table>" discard)
    '(html nil (body nil (p nil "A") (div nil (svg ((viewbox . "0 0 1 1")) (circle nil))) "BC" (table nil "D" (tr nil (td nil "E")) "F")))))

;; Parser bookkeeping must be per call, including the EOF text flush.
(dotimes (_ 3)
  (qa (qa-html-repair--parse "<p>A<div>B</div>C</p>D")
    '(html nil (body nil (p nil "A") (div nil "B") "CD")))
  (qa (qa-html-repair--parse "<p></p>")
    '(html nil (body nil (p nil))))
  (qa (qa-html-repair--parse "<table>A")
    '(html nil (body nil (table nil "A")))))

(princ (qa--done))
(terpri)
