;; Probe: format/print specifier coverage.
(progn
  ;; %s / %S / precision / width / alignment
  (prin1 (format "%s %S" 'sym "str"))
  (prin1 (format "%5s|%-5s|" 'a 'b))
  (prin1 (format "%.2s" 'abcdef))
  (prin1 (format "%.3S" '(1 2 3 4 5)))
  (prin1 (format "%08.3s" 'xyz))
  ;; positional args
  (prin1 (format "%2$s %1$s" 'first 'second))
  (prin1 (format "%3$d %1$d" 1 2 3))
  ;; %d on int/float/marker
  (prin1 (format "%d %d" 7 2.9))
  (with-temp-buffer
    (insert "xy")
    (prin1 (format "%d" (point-marker))))
  (condition-case e (format "%d" "s") (error (prin1 (car e))))
  ;; flags
  (prin1 (format "%+d %+d % d % d" 3 -3 4 -4))
  (prin1 (format "%05d %-5d|" -42 7))
  (prin1 (format "%#o %#x" 8 255))
  ;; %c char
  (prin1 (format "%c%c" 65 97))
  ;; %o %x %X
  (prin1 (format "%o %x %X" 64 255 255))
  (prin1 (format "%x" -1))
  (condition-case e (format "%x" "s") (error (prin1 (car e))))
  ;; %e %f %g
  (prin1 (format "%e" 1234.5))
  (prin1 (format "%E" 1234.5))
  (prin1 (format "%.2e" 0.001234))
  (prin1 (format "%f %.2f %8.2f" 3.14159 3.14159 3.14159))
  (prin1 (format "%g %g %g" 0.00001 123456789.0 3.5))
  (prin1 (format "%.10g" 3.14159265358979))
  ;; literal % and trailing %
  (prin1 (format "100%%"))
  (prin1 (format "end%"))
  ;; error on unknown spec
  (condition-case e (format "%z" 1) (error (prin1 (car e))))
  (condition-case e (format "%s") (error (prin1 (car e))))
  ;; format-spec / format-message
  (prin1 (format-spec "%a-%b-%z" '((?a . "A") (?b . 2))))
  (prin1 (format-spec "%5a|%-5b|" '((?a . "x") (?b . "y"))))
  (prin1 (format-message "a `x' %s" 1))
  ;; prin1/princ via format
  (prin1 (format "%S" "esc\nape"))
  (prin1 (format "%s" "esc\nape"))
  (prin1 'done))
