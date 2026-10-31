use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'net-utils)
  (dolist (name '(nslookup--ipv4-address-p nslookup-host-ipv4
                  nslookup--ipv6-expand nslookup-host-ipv6 nslookup ping))
    (unless (fboundp name) (error "Missing net-utils definition: %S" name)))
  (dolist (address '("0.0.0.0" "10.20.30.40" "192.0.2.5" "255.255.255.255"))
    (unless (nslookup--ipv4-address-p address)
      (error "Valid IPv4 address rejected: %S" address)))
  (dolist (address '(nil 1 "" "192.0.2" "192.0.2.5.6" "256.0.2.5"
                    "-1.0.2.5" "a.0.2.5" "192..2.5" "192.0.2.5\n"
                    "192.168.010.010" "01.2.3.4" "1.02.3.4" "1.2.03.4"
                    "1.2.3.04" "00.0.0.0" "001.002.003.004"))
    (when (nslookup--ipv4-address-p address)
      (error "Invalid IPv4 address accepted: %S" address)))
  (cl-letf (((symbol-function 'nslookup--query) (lambda (&rest _) "")))
    (dolist (format '(nil string vector))
      (unless (null (nslookup-host-ipv4 "192.168.010.010" nil format))
        (error "Lookup accepted a leading-zero IPv4 literal"))))
  (cl-letf (((symbol-function 'nslookup--query)
             (lambda (&rest _) "Name: example.test\nAddress: 192.168.010.010\n")))
    (dolist (format '(nil string vector))
      (unless (null (nslookup-host-ipv4 "example.test" nil format))
        (error "Lookup accepted leading-zero IPv4 output"))))
  (let ((ping-program "ping") (ping-program-options '("-c" "2")))
    (cl-letf (((symbol-function 'net-utils-run-program) #'list))
      (unless (equal (nth 3 (ping "example.test")) '("-c" "2" "example.test"))
        (error "Ping default options are not usable"))
      (unless (equal (nth 3 (ping "example.test" '("-n"))) '("-n" "example.test"))
        (error "Ping explicit options are not usable"))))
  (let ((system-type 'darwin))
    (cl-letf (((symbol-function 'process-file)
               (lambda (_program _infile _destination _display &rest arguments)
                 (insert (if (equal (car arguments) "-type=AAAA")
                             "example.test has AAAA address 2001:db8::5\n"
                           "Name: example.test\nAddress: 192.0.2.5\n"))
                 0)))
      (unless (equal (nslookup-host-ipv4 "example.test") "192.0.2.5")
        (error "IPv4 lookup definition is not usable"))
      (unless (equal (nslookup-host-ipv4 "example.test" nil 'vector) [192 0 2 5])
        (error "IPv4 vector conversion is not usable"))
      (unless (equal (nslookup-host-ipv6 "example.test") "2001:db8::5")
        (error "IPv6 lookup definition is not usable"))
      (unless (equal (nslookup-host-ipv6 "example.test" nil 'vector)
                     [8193 3512 0 0 0 0 0 5])
        (error "IPv6 vector conversion is not usable"))))
  (let ((nslookup-program "/path with spaces/nslookup")
        (host "x; printf 'Name: x\\nAddress: 192.0.2.7\\n'")
        (server "server $(printf injected) 'quoted'\nmore")
        calls)
    (cl-letf (((symbol-function 'shell-command-to-string)
               (lambda (&rest _) (error "DNS lookup invoked a shell")))
              ((symbol-function 'process-file)
               (lambda (&rest arguments)
                 (push arguments calls)
                 (insert "raw output")
                 1)))
      (dolist (type '("A" "AAAA"))
        (dolist (name-server (list nil server))
          (unless (equal (nslookup--query type host name-server) "raw output")
            (error "Lookup lost subprocess output"))
          (unless (equal (pop calls)
                         (append (list nslookup-program nil t nil
                                       (concat "-type=" type) host)
                                 (and name-server (list name-server))))
            (error "Lookup changed literal process arguments"))))))
  ;; Exercise the real subprocess boundary with harmless injection payloads.
  (let ((nslookup-program (executable-find "true")))
    (when nslookup-program
      (dolist (type '("A" "AAAA"))
        (dolist (payload '("x; printf 'Name: x\\nAddress: 192.0.2.7\\n'"
                           "x; printf 'has AAAA address ::1\\n'"))
          (unless (and (equal (nslookup--query type payload nil) "")
                       (equal (nslookup--query type "example.test" payload) ""))
            (error "Lookup executed injected shell syntax"))))))
  t)
"#;

#[test]
fn net_utils_loads_and_installs_callable_dns_definitions() {
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
