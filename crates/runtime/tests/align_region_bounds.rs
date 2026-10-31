use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'cl-lib)
  (require 'align)
  (let ((rules '((test (regexp . "\\( *= *\\)"))))
        (separator "^$"))
    (cl-labels ((values-aligned-p (beg end)
                  (let (columns)
                    (save-excursion
                      (goto-char beg)
                      (while (re-search-forward "[0-9]+" end t)
                        (push (save-excursion
                                (goto-char (match-beginning 0))
                                (current-column))
                              columns)))
                    (and (> (length columns) 1)
                         (apply #'= columns)))))
      ;; A supplied BEG with omitted END must stop at the first divider.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n\nx  = 3\nz=4\n")
        (let* ((section-end
                (copy-marker (save-excursion
                               (goto-char (point-min))
                               (search-forward "\n\n")
                               (point)) t))
               (suffix (buffer-substring-no-properties section-end (point-max))))
          (align-region (point-min) nil separator rules nil)
          (unless (values-aligned-p (point-min) section-end)
            (error "BEG with nil END did not align the first section"))
          (unless (equal suffix
                         (buffer-substring-no-properties section-end (point-max)))
            (error "BEG with nil END crossed the section divider"))))
      ;; An explicit END still permits every section in the requested range.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n\nx  = 3\nz=4\n")
        (let ((section-end
               (copy-marker (save-excursion
                              (goto-char (point-min))
                              (search-forward "\n\n")
                              (point)) t)))
          (align-region (point-min) (point-max) separator rules nil)
          (unless (and (values-aligned-p (point-min) section-end)
                       (values-aligned-p section-end (point-max)))
            (error "Explicit BEG and END did not align both sections"))))
      ;; An omitted BEG uses point to find the start, without replacing END.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n\nx  = 3\nz=4\n")
        (let ((section-end
               (copy-marker (save-excursion
                              (goto-char (point-min))
                              (search-forward "\n\n")
                              (point)) t)))
          (goto-char (point-min))
          (align-region nil (point-max) separator rules nil)
          (unless (and (values-aligned-p (point-min) section-end)
                       (values-aligned-p section-end (point-max)))
            (error "Explicit END was replaced when BEG was omitted"))))
      ;; Each rule with its own separator gets an independent omitted END.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n---\nx: 1\nlong: 2\n\nwide: 3\nvery-long: 4\n---\nafter = 5\nfarther-name = 6\n")
        (let* ((other-rules
                '((equals (regexp . "\\( *= *\\)") (separate . "^---$"))
                  (colon (regexp . "\\( *: *\\)") (separate . "^$"))))
               (colon-beg
                (copy-marker (save-excursion
                               (goto-char (point-min))
                               (search-forward "---\n")
                               (point))))
               (colon-end
                (copy-marker (save-excursion
                               (goto-char colon-beg)
                               (search-forward "\n\n")
                               (point)) t))
               (suffix (buffer-substring-no-properties colon-end (point-max))))
          (align-region (point-min) nil separator other-rules nil)
          (unless (values-aligned-p colon-beg colon-end)
            (error "Rule-specific section end reused another rule's boundary"))
          (unless (equal suffix
                         (buffer-substring-no-properties colon-end (point-max)))
            (error "Rule-specific omitted END crossed its separator"))))
      ;; FUNC and recursive exclusion searches receive the resolved bound.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n\nx  = 3\nz=4\n")
        (let* ((next-section
                (copy-marker (save-excursion
                               (goto-char (point-min))
                               (search-forward "\n\n")
                               (point))))
               (callback-bounds nil))
          (cl-labels ((check-exclusion-bound (limit reverse)
                        (unless reverse
                          (let ((limit-position
                                 (if (markerp limit)
                                     (marker-position limit)
                                   limit)))
                            (when (>= limit-position next-section)
                              (error "Exclude rule escaped the resolved section"))))
                        (re-search-forward "never-matches" limit t)))
            (align-region
             (point-min) nil separator rules
             (list (list 'suffix (cons 'regexp #'check-exclusion-bound)))
             (lambda (beg end mode)
               (when (consp mode)
                 (push (cons beg end) callback-bounds)
                 t)))
            (unless (and (= (length callback-bounds) 1)
                         (= (caar callback-bounds) (point-min))
                         (integerp (cdar callback-bounds))
                         (< (cdar callback-bounds) next-section))
              (error "FUNC did not receive the resolved first-section bounds")))))
      ;; With both bounds omitted, align-region keeps its point-based section.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n\nx  = 3\nz=4\n")
        (let* ((section-end
                (copy-marker (save-excursion
                               (goto-char (point-min))
                               (search-forward "\n\n")
                               (point)) t))
               (suffix (buffer-substring-no-properties section-end (point-max))))
          (goto-char (point-min))
          (align-region nil nil separator rules nil)
          (unless (and (values-aligned-p (point-min) section-end)
                       (equal suffix
                              (buffer-substring-no-properties section-end (point-max))))
            (error "Unbounded align-region changed its current section"))))
      ;; No divider means an omitted END extends to the accessible buffer end.
      (with-temp-buffer
        (insert "a = 1\nlong-name = 2\n")
        (align-region (point-min) nil separator rules nil)
        (unless (values-aligned-p (point-min) (point-max))
          (error "No divider did not extend alignment to buffer end")))
      ;; The accessible end while narrowed is the limit for an omitted END.
      (with-temp-buffer
        (insert "outside=0\na = 1\nlong-name = 2\noutside=3\n")
        (let* ((inner-beg (copy-marker
                           (+ (point-min) (length "outside=0\n"))))
               (inner-end (copy-marker
                           (- (point-max) (length "outside=3\n")) t))
               (prefix (buffer-substring-no-properties (point-min) inner-beg))
               (suffix (buffer-substring-no-properties inner-end (point-max))))
          (narrow-to-region inner-beg inner-end)
          (align-region (point-min) nil separator rules nil)
          (unless (values-aligned-p (point-min) (point-max))
            (error "Narrowed section was not aligned"))
          (widen)
          (unless (and (equal prefix
                              (buffer-substring-no-properties (point-min) inner-beg))
                       (equal suffix
                              (buffer-substring-no-properties inner-end (point-max))))
            (error "Alignment crossed the narrowing bounds")))))
  t))
"#;

#[test]
fn align_region_resolves_omitted_end_from_its_section() {
    let mut interp = Interp::new();
    match interp.eval_str(CASES) {
        Ok(value) => assert!(value.truthy()),
        Err(Flow::Signal(signal)) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        Err(other) => panic!("{other:?}"),
    }
}
