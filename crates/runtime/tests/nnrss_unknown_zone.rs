use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (unless (and (featurep 'nnrss)
               (featurep 'timezone)
               (fboundp 'nnrss-normalize-date)
               (fboundp 'timezone-parse-date))
    (error "Full nnrss/timezone source load did not establish its features"))
  (defun remacs-nnrss2995-assert (condition message)
    (unless condition (error "%s" message)))
  (defun remacs-nnrss2995-check-zone (label date expected)
    (let* ((normalized (nnrss-normalize-date date))
           (suffix (and (stringp normalized)
                        (>= (length normalized) 5)
                        (substring normalized (- (length normalized) 5)))))
      (remacs-nnrss2995-assert
       (equal suffix expected)
       (format "Zone normalization %S: %S -> %S, expected %S"
               label date suffix expected))))
  (defun remacs-nnrss2995-check-no-zone (label date)
    (let ((normalized (nnrss-normalize-date date)))
      (remacs-nnrss2995-assert
       (and (stringp normalized)
            (not (string-match-p "[-+][0-9][0-9][0-9][0-9]$" normalized)))
       (format "Zone-less normalization %S added a zone: %S -> %S"
               label date normalized))))
  (let ((case-fold-search nil))
    ;; Numeric offsets retain their exact sign and minute component.
    (remacs-nnrss2995-check-zone
     'numeric-positive "Thu, 21 Dec 2000 16:01:07 +0200" "+0200")
    (remacs-nnrss2995-check-zone
     'numeric-negative "Wed, 09 Jun 2021 10:18:14 -0430" "-0430")
    (remacs-nnrss2995-check-zone
     'negative-zero-preserves-unknown "Wed, 09 Jun 2021 10:18:14 -0000" "-0000")
    (remacs-nnrss2995-check-zone
     'positive-zero-preserves-utc "Wed, 09 Jun 2021 10:18:14 +0000" "+0000")
    ;; UTC aliases and the timezone library's regional table keep their offsets.
    (remacs-nnrss2995-check-zone
     'utc-alias "Wed, 09 Jun 2021 10:18:14 UTC" "+0000")
    (remacs-nnrss2995-check-zone
     'lowercase-utc-alias "Wed, 09 Jun 2021 10:18:14 utc" "+0000")
    (remacs-nnrss2995-check-zone
     'ut-alias "Wed, 09 Jun 2021 10:18:14 UT" "+0000")
    (remacs-nnrss2995-check-zone
     'gmt-alias "Wed, 09 Jun 2021 10:18:14 GMT" "+0000")
    (remacs-nnrss2995-check-zone
     'lowercase-gmt-alias "Wed, 09 Jun 2021 10:18:14 gmt" "+0000")
    (remacs-nnrss2995-check-zone
     'pacific-standard "Wed, 09 Jun 2021 10:18:14 PST" "-0800")
    (remacs-nnrss2995-check-zone
     'pacific-daylight "Wed, 09 Jun 2021 10:18:14 PDT" "-0700")
    (remacs-nnrss2995-check-zone
     'eastern-standard "Wed, 09 Jun 2021 10:18:14 EST" "-0500")
    (remacs-nnrss2995-check-zone
     'eastern-daylight "Wed, 09 Jun 2021 10:18:14 EDT" "-0400")
    (remacs-nnrss2995-check-zone
     'atlantic-standard "Wed, 09 Jun 2021 10:18:14 AST" "-0400")
    (remacs-nnrss2995-check-zone
     'newfoundland-standard "Wed, 09 Jun 2021 10:18:14 NST" "-0330")
    (remacs-nnrss2995-check-zone
     'british-summer "Wed, 09 Jun 2021 10:18:14 BST" "+0100")
    (remacs-nnrss2995-check-zone
     'middle-europe "Wed, 09 Jun 2021 10:18:14 MET" "+0100")
    (remacs-nnrss2995-check-zone
     'eastern-europe "Wed, 09 Jun 2021 10:18:14 EET" "+0200")
    (remacs-nnrss2995-check-zone
     'japan "Wed, 09 Jun 2021 10:18:14 JST" "+0900")
    (remacs-nnrss2995-check-zone
     'gmt-plus "Wed, 09 Jun 2021 10:18:14 GMT+1" "+0100")
    (remacs-nnrss2995-check-zone
     'gmt-minus "Wed, 09 Jun 2021 10:18:14 GMT-1" "-0100")
    ;; RFC 5322 §4.3 says unknown obsolete alphabetic zones map to -0000.
    (remacs-nnrss2995-check-zone
     'one-letter-military "Wed, 09 Jun 2021 10:18:14 A" "-0000")
    (remacs-nnrss2995-check-zone
     'zulu-military-obsolete "Wed, 09 Jun 2021 10:18:14 Z" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-two-letters "Wed, 09 Jun 2021 10:18:14 XY" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-three-letters "Wed, 09 Jun 2021 10:18:14 XYZ" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-cet "Wed, 09 Jun 2021 10:18:14 CET" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-five-letters "Wed, 09 Jun 2021 10:18:14 ABCDE" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-long-token "Wed, 09 Jun 2021 10:18:14 UNKNOWN" "-0000")
    (remacs-nnrss2995-check-zone
     'unknown-lowercase "Wed, 09 Jun 2021 10:18:14 xyz" "-0000")
    ;; Missing zone remains distinct from an unknown zone designation.
    (remacs-nnrss2995-check-no-zone
     'rfc-zone-omitted "Wed, 09 Jun 2021 10:18:14")
    ;; ISO timezone handling remains on its separate branch.
    (remacs-nnrss2995-check-zone 'iso-z "2021-06-09T10:18:14Z" "+0000")
    (remacs-nnrss2995-check-zone
     'iso-offset "2021-06-09T10:18:14+01:30" "+0130")
    (remacs-nnrss2995-check-no-zone
     'iso-zone-omitted "2021-06-09T10:18:14"))
  t)"###;

fn lisp_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

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
fn nnrss_normalizes_unknown_rfc_zone_after_full_source_load_2995() {
    let source_root = PathBuf::from(env!("REMACS_SOURCE_ROOT"));
    let mut interp = Interp::new();
    eval(&mut interp, "(setq noninteractive t)");
    for relative in ["lisp/nnrss.el", "lisp/timezone.el"] {
        let source = source_root.join(relative);
        eval(
            &mut interp,
            &format!("(load {} nil t t)", lisp_string(source.to_str().unwrap())),
        );
    }
    assert_eq!(eval(&mut interp, CASES), "t");
}
