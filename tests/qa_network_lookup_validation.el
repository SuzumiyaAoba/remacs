;;; -*- lexical-binding: nil; -*-
;; #633 #634: validation and numeric lookup must not trigger DNS for names.
(qa (network-lookup-address-info "127.0.0.1" 'ipv4 'numeric) '([127 0 0 1 0]))
(qa (network-lookup-address-info "::1" 'ipv6 'numeric) '([0 0 0 0 0 0 0 1 0]))
(qa (network-lookup-address-info "localhost" nil 'numeric) nil)
(qa (condition-case error
         (network-interface-list nil (make-symbol "ipv4"))
       (error (list (car error) (cadr error))))
    '(error "Unsupported address family"))
(defun qa633-network-option-error (type family)
  (let (process)
    (unwind-protect
        (condition-case error
            (progn
              (setq process
                    (make-network-process
                     :name "qa633-network-option-identity"
                     :type type :family family :server t :host "127.0.0.1"
                     :service 0 :noquery t))
              'accepted)
          (error (car error)))
      (when process (delete-process process)))))
(defun qa633-network-option-error-data (type family)
  (let (process)
    (unwind-protect
        (condition-case error
            (progn
              (setq process
                    (make-network-process
                     :name "qa633-network-option-error-data"
                     :type type :family family :server t :host "127.0.0.1"
                     :service 0 :noquery t))
              'accepted)
          (error (list (car error) (cadr error))))
      (when process (delete-process process)))))
(defun qa633-network-explicit-address-error-data (server address)
  (let ((address-key (if server :local :remote)) process)
    (unwind-protect
        (condition-case error
            (progn
              (setq process
                    (apply #'make-network-process
                           (append (list :name "qa633-network-explicit-address"
                                         :server server
                                         :family (make-symbol "invalid-family")
                                         :host 7 :service 7 :noquery t)
                                   (list address-key address))))
              'accepted)
          (error (list (car error) (cadr error))))
      (when process (delete-process process)))))
;; Network types and address families are symbols compared by identity.
(qa (qa633-network-option-error (make-symbol "datagram") 'ipv4) 'error)
(qa (qa633-network-option-error 'datagram (make-symbol "ipv4")) 'error)
(qa (qa633-network-option-error nil (make-symbol "ipv4")) 'error)
(qa (qa633-network-option-error nil (make-symbol "local")) 'error)
;; Integer address families are passed to getaddrinfo; unsupported values
;; must fail before a socket bind is attempted.
(qa (qa633-network-option-error nil 7) 'error)
;; :family accepts only nil, a supported symbol, or a C-int-sized integer.
(qa (qa633-network-option-error nil "ipv4") 'error)
(qa (qa633-network-option-error nil 9223372036854775808) 'error)
;; `make-network-process' accepts only family fixnums representable as C int.
(qa (qa633-network-option-error-data nil 2147483648)
    '(error "Unknown address family"))
(qa (qa633-network-option-error-data 'datagram 2147483648)
    '(error "Unknown address family"))
;; Integer AF_UNIX selects the local path before service type validation.
(qa (qa633-network-option-error nil 1) 'wrong-type-argument)
(qa (qa633-network-option-error 'datagram 1) 'wrong-type-argument)
;; Explicit addresses are validated before family, host, and service options.
(qa (qa633-network-explicit-address-error-data nil [1 2 3])
    '(error "Malformed :address"))
(qa (qa633-network-explicit-address-error-data t [1 2 3])
    '(error "Malformed :address"))
(dolist (hint (list t 'unsupported 3 "numeric" (make-symbol "numeric")))
  (qa (condition-case error (network-lookup-address-info "127.0.0.1" nil hint)
        (error error))
      '(error "Unsupported hints value")))
(dolist (code '(233 #xf0080 #x110000 #x3fff80))
  (qa (let ((name (string code)))
        (list (condition-case error (network-lookup-address-info name nil 'numeric)
                (error (car error)))
              (condition-case error (make-network-process :name "qa633" :host name :service 1)
                (error (car error)))))
      '(error error)))
(qa (condition-case error (network-lookup-address-info (string 233) nil 'numeric)
      (error error))
    '(error "Non-ASCII hostname é detected, please use ‘puny-encode-domain’"))
(qa (condition-case error (network-lookup-address-info (string 233) 'bad-family 'bad-hint)
      (error error))
    '(error "Unsupported family"))
(qa (condition-case error (network-lookup-address-info (string 233) nil 'bad-hint)
      (error error))
    '(error "Unsupported hints value"))
(qa (network-lookup-address-info (unibyte-string 233) nil 'numeric) nil)
;; A cached C symbol remains the accepted hint after removal from obarray.
(qa (let ((hint 'numeric))
      (unintern hint obarray)
      (network-lookup-address-info "127.0.0.1" 'ipv4 hint))
    '([127 0 0 1 0]))
(princ (qa--done))
