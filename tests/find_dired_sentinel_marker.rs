mod common;

#[test]
fn find_dired_sentinel_refines_from_the_saved_output_start() {
    assert_eq!(
        common::ev(
            r#"(progn
                 (require 'cl-lib)
                 (require 'find-dired)
                 (defvar qa2664-observed nil)
                 (fset 'qa2664-refiner
                       (lambda ()
                         (setq qa2664-observed
                               (split-string
                                (buffer-substring-no-properties
                                 (point-min) (point-max))
                                "\n" t)))
                 (let ((find-dired-refine-function #'qa2664-refiner)
                       (test-buffer
                        (generate-new-buffer " *find-dired-sentinel-test*"))
                       marker)
                   (unwind-protect
                       (progn
                         (with-current-buffer test-buffer
                           (insert "  /tmp:\n  find . -ls\n  z.txt\n  a.txt\n  ")
                           (setq marker
                                 (copy-marker
                                  (save-excursion
                                    (goto-char (point-min))
                                    (forward-line 2)
                                    (point))))
                           (goto-char (point-min))
                           (cl-letf (((symbol-function 'process-buffer)
                                      (lambda (_proc) test-buffer))
                                     ((symbol-function 'process-get)
                                      (lambda (_proc property)
                                        (and (eq property 'find-dired-output-start)
                                             marker)))
                                     ((symbol-function 'process-status)
                                      (lambda (_proc) 'exit))
                                     ((symbol-function 'delete-process)
                                      (lambda (_proc) nil))
                                     ((symbol-function 'dired-insert-set-properties)
                                      (lambda (&rest _args) nil)))
                             (find-dired-sentinel 'qa-fake-process "finished\n")))
                         qa2664-observed)
                     (when (buffer-live-p test-buffer)
                       (kill-buffer test-buffer)))))"#
        ),
        "(\"  z.txt\" \"  a.txt\")"
    );
}
