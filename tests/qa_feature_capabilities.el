;;; -*- lexical-binding: t; -*-
;; Issue #109: native compilation has one canonical feature name.
;; Optional library features describe this runtime's actual capabilities,
;; so a reference build without those libraries may legitimately differ.
(defvar qa109-feature)
(qa (featurep 'native-comp) nil)
(qa (memq 'native-comp features) nil)
(qa (featurep 'native-compile) t)
(qa (if (native-comp-available-p) (featurep 'native-compile) t) t)
(qa (if (featurep 'native-compile) (fboundp 'native-compile) t) t)
(qa (eq (featurep 'lcms2)
        (and (fboundp 'lcms2-available-p) (lcms2-available-p))) t)
(qa (if (featurep 'dbusbind)
        (and (boundp 'dbus-compiled-version) (stringp dbus-compiled-version)
             (boundp 'dbus-runtime-version) (stringp dbus-runtime-version))
      t) t)
(dolist (qa109-feature '(native-compile dbusbind lcms2))
  (qa (let ((count 0))
        (dolist (feature features)
          (when (eq feature qa109-feature) (setq count (1+ count))))
        count)
      (if (featurep qa109-feature) 1 0)))

(princ (qa--done)) (terpri)
