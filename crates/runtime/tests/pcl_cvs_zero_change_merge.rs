use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"(progn
  (require 'pcvs-parse)
  ;; This variable is dynamically bound by cvs-parse-buffer in pcvs-parse.el.
  (defvar dont-change-disc nil)
  (defun remacs-pcl-b18-assert (condition label actual)
    (unless condition (error "PCL-CVS #1706 %s: %S" label actual)))
  (defun remacs-pcl-b18-parse-merge (text dcd)
    (with-temp-buffer
      (insert text)
      (car (cvs-parse-buffer #'cvs-parse-merge dcd ""))))
  (defun remacs-pcl-b18-state (fileinfo)
    (list (cvs-fileinfo->type fileinfo)
          (cvs-fileinfo->subtype fileinfo)
          (cvs-fileinfo->base-rev fileinfo)
          (cvs-fileinfo->merge fileinfo)))

  (let* ((prefix (concat "RCS file: /repo/foo,v\n"
                         "retrieving revision 1.1\n"
                         "retrieving revision 1.2\n"
                         "Merging differences between 1.1 and 1.2 into foo\n"))
         (zero-change
          (concat prefix
                  "foo already contains the differences between 1.1 and 1.2\n"))
         (unchanged (remacs-pcl-b18-parse-merge zero-change nil))
         (modified (remacs-pcl-b18-parse-merge
                    (concat prefix "M foo\n") nil))
         (conflict (remacs-pcl-b18-parse-merge
                    (concat prefix "C foo\n") nil))
         (examine (remacs-pcl-b18-parse-merge zero-change t)))
    (remacs-pcl-b18-assert
     (equal (remacs-pcl-b18-state unchanged)
            '(MODIFIED MERGED nil ("1.1" . "1.2")))
     'zero-change (remacs-pcl-b18-state unchanged))
    (remacs-pcl-b18-assert
     (and (eq (cvs-fileinfo->type modified) 'MODIFIED)
          (eq (cvs-fileinfo->subtype modified) 'MERGED))
     'ordinary-merge (remacs-pcl-b18-state modified))
    (remacs-pcl-b18-assert
     (eq (cvs-fileinfo->type conflict) 'CONFLICT)
     'conflict (remacs-pcl-b18-state conflict))
    (remacs-pcl-b18-assert
     (eq (cvs-fileinfo->type examine) 'NEED-MERGE)
     'dont-change-disc (remacs-pcl-b18-state examine)))
  t)"##;

fn eval(interp: &mut Interp, source: &str) -> String {
    let value = interp.eval_str(source).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    format!("{}", interp.prin1_to_string(&value))
}

#[test]
fn pcl_cvs_zero_change_merge_keeps_modified_working_copy_status() {
    let mut interp = Interp::new_with_noninteractive(true);
    assert_eq!(eval(&mut interp, CASES), "t");
}
