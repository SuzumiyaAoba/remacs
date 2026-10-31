use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'net-utils)
  (dolist (name '(nslookup--ipv4-address-p nslookup-host-ipv4
                  nslookup--ipv6-expand nslookup-host-ipv6 nslookup ping))
    (unless (fboundp name) (error "Missing net-utils definition: %S" name)))
  (dolist (address '("0.0.0.0" "192.0.2.5" "255.255.255.255"))
    (unless (nslookup--ipv4-address-p address)
      (error "Valid IPv4 address rejected: %S" address)))
  (dolist (address '(nil 1 "" "192.0.2" "192.0.2.5.6" "256.0.2.5"
                    "-1.0.2.5" "a.0.2.5" "192..2.5" "192.0.2.5\n"))
    (when (nslookup--ipv4-address-p address)
      (error "Invalid IPv4 address accepted: %S" address)))
  (let ((ping-program "ping") (ping-program-options '("-c" "2")))
    (cl-letf (((symbol-function 'net-utils-run-program) #'list))
      (unless (equal (nth 3 (ping "example.test")) '("-c" "2" "example.test"))
        (error "Ping default options are not usable"))
      (unless (equal (nth 3 (ping "example.test" '("-n"))) '("-n" "example.test"))
        (error "Ping explicit options are not usable"))))
  (let ((system-type 'darwin))
    (cl-letf (((symbol-function 'shell-command-to-string)
               (lambda (command)
                 (if (string-match-p "-type=AAAA" command)
                     "example.test has AAAA address 2001:db8::5\n"
                   "Name: example.test\nAddress: 192.0.2.5\n"))))
      (unless (equal (nslookup-host-ipv4 "example.test") "192.0.2.5")
        (error "IPv4 lookup definition is not usable"))
      (unless (equal (nslookup-host-ipv4 "example.test" nil 'vector) [192 0 2 5])
        (error "IPv4 vector conversion is not usable"))
      (unless (equal (nslookup-host-ipv6 "example.test") "2001:db8::5")
        (error "IPv6 lookup definition is not usable"))
      (unless (equal (nslookup-host-ipv6 "example.test" nil 'vector)
                     [8193 3512 0 0 0 0 0 5])
        (error "IPv6 vector conversion is not usable"))))
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
