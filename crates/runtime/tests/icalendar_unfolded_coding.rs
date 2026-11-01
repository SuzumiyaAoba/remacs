use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r##"
(progn
  (require 'icalendar-parser)
  (defconst remacs-test-ascii-calendar
    (concat "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//test//EN\r\n"
            "BEGIN:VEVENT\r\nUID:coding@example.test\r\nDTSTAMP:20250101T000000Z\r\n"
            "DTSTART;VALUE=DATE:20250101\r\nSUMMARY:Alpha\r\n beta\r\n"
            "END:VEVENT\r\nEND:VCALENDAR\r\n"))
  (defun remacs-test-unfolded-file (source write-coding summary prefer-utf8)
    (let ((file (make-temp-file "remacs-ical-coding-" nil ".ics"))
          (output (make-temp-file "remacs-ical-output-" nil ".ics"))
          buffer)
      (unwind-protect
          (progn
            (let ((coding-system-for-write write-coding))
              (write-region source nil file nil 'silent))
            (let ((coding-system-for-read nil))
              (setq buffer (icalendar-unfolded-buffer-from-file file)))
            (with-current-buffer buffer
              (unless (if prefer-utf8
                          (eq buffer-file-coding-system 'prefer-utf-8-dos)
                        (and (eq (coding-system-type buffer-file-coding-system) 'utf-8)
                             (eq (coding-system-eol-type buffer-file-coding-system) 1)))
                (error "Wrong unfolded file coding: %S" buffer-file-coding-system))
              (unless (and (string-search (concat "SUMMARY:" summary "\n") (buffer-string))
                           (not (string-search "\r" (buffer-string)))
                           (not (buffer-modified-p)))
                (error "File unfolding/decoding changed text or modification state"))
              (when prefer-utf8
                (goto-char (point-min))
                (search-forward summary)
                (replace-match "日本語" t t)
                (let ((coding-system-for-write buffer-file-coding-system))
                  (write-region (point-min) (point-max) output nil 'silent))
                (with-temp-buffer
                  (set-buffer-multibyte nil)
                  (insert-file-contents-literally output)
                  (let ((bytes (buffer-string)))
                    (unless (and (string-search (unibyte-string 230 151 165 230 156 172 232 170 158)
                                                bytes)
                                 (string-search "\r\n" bytes)
                                 (not (string-match-p "\\(?:\\`\\|[^\r]\\)\n" bytes)))
                      (error "UTF-8 preference did not write UTF-8 bytes with CRLF"))))))
            (unless (eq buffer (icalendar-unfolded-buffer-from-file file))
              (error "A cached unfolded file lost its buffer identity")))
        (when (buffer-live-p buffer)
          (with-current-buffer buffer (set-buffer-modified-p nil))
          (kill-buffer buffer))
        (delete-file file)
        (delete-file output))))
  (remacs-test-unfolded-file remacs-test-ascii-calendar 'no-conversion "Alphabeta" t)
  (remacs-test-unfolded-file
   (replace-regexp-in-string "Alpha\r\n beta" "日本語\r\n 続き" remacs-test-ascii-calendar t t)
   'utf-8-unix "日本語続き" nil)
  ;; The region constructor uses the same coding policy for already-decoded text.
  (dolist (case '((nil prefer-utf-8-dos) (undecided prefer-utf-8-dos)
                  (undecided-unix prefer-utf-8-dos) (undecided-dos prefer-utf-8-dos)
                  (undecided-mac prefer-utf-8-dos) (utf-8-unix utf-8-dos)
                  (iso-latin-1-unix iso-latin-1-dos)))
    (let (unfolded)
      (unwind-protect
          (with-temp-buffer
            (insert "SUMMARY:Alpha\n beta\n")
            (let ((last-coding-system-used (car case)))
              (setq unfolded (icalendar-unfolded-buffer-from-region (point-min) (point-max))))
            (with-current-buffer unfolded
              (unless (and (eq buffer-file-coding-system (cadr case))
                           (equal (buffer-string) "SUMMARY:Alphabeta\n"))
                (error "Wrong unfolded region coding for %S: %S" (car case) buffer-file-coding-system))))
        (when (buffer-live-p unfolded) (kill-buffer unfolded)))))
  t)
"##;

#[test]
fn unfolding_prefers_utf8_for_undecided_input_and_keeps_detected_encodings() {
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
