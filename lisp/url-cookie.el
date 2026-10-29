;;; url-cookie.el --- URL cookie support  -*- lexical-binding:t -*-

;; Copyright (C) 1996-1999, 2004-2026 Free Software Foundation, Inc.

;; Keywords: comm, data, processes, hypermedia

;; This file is part of GNU Emacs.
;;
;; GNU Emacs is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; GNU Emacs is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with GNU Emacs.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;;; Code:

(require 'url-util)
(require 'url-parse)
(require 'url-domsuf)
(require 'generate-lisp-file)

(eval-when-compile (require 'cl-lib))

(defgroup url-cookie nil
  "URL cookies."
  :prefix "url-"
  :prefix "url-cookie-"
  :group 'url)

;; A cookie is stored internally as a vector of 7 slots
;; [ url-cookie NAME VALUE EXPIRES LOCALPART DOMAIN SECURE ]

(cl-defstruct (url-cookie
            (:constructor url-cookie-create)
            (:copier nil)
            (:type vector)
            :named)
  name value expires localpart domain secure)

(defvar url-cookie-storage nil         "Where cookies are stored.")
(defvar url-cookie-secure-storage nil  "Where secure cookies are stored.")
(defcustom url-cookie-file nil
  "File where cookies are stored on disk."
  :type '(choice (const :tag "Default" :value nil) file)
  :group 'url-file
  :group 'url-cookie)

(defcustom url-cookie-confirmation nil
  "If non-nil, confirmation by the user is required to accept HTTP cookies."
  :type 'boolean)

(defcustom url-cookie-multiple-line nil
  "If nil, HTTP requests put all cookies for the server on one line.
Some web servers, such as https://www.hotmail.com/, only accept cookies
when they are on one line.  This is broken behavior, but just try
telling Microsoft that."
  :type 'boolean)

(defvar url-cookies-changed-since-last-save nil
  "Whether the cookies list has changed since the last save operation.")

(defun url-cookie-parse-file (&optional fname)
  "Load FNAME, default `url-cookie-file'."
  ;; It's completely normal for the cookies file not to exist yet.
  (let ((warning-inhibit-types '((files missing-lexbind-cookie))))
    (load (or fname url-cookie-file) t t)))

(defun url-cookie-parse-file-netscape (filename &optional long-session)
  "Load cookies from FILENAME in Netscape/Mozilla format.
When LONG-SESSION is non-nil, session cookies (expiring at t=0
i.e. 1970-1-1) are loaded as expiring one year from now instead."
  (interactive "fLoad Netscape/Mozilla cookie file: ")
  (let ((n 0))
    (with-temp-buffer
      (insert-file-contents-literally filename)
      ;; Netscape cookie files are commonly written with CRLF line endings.
      (goto-char (point-min))
      (while (search-forward "\r\n" nil t)
	(replace-match "\n" t t))
      (goto-char (point-min))
      (when (not (looking-at-p "# Netscape HTTP Cookie File\n"))
	(error (format "File %s doesn't look like a netscape cookie file" filename)))
      (while (not (eobp))
	(when (not (looking-at-p (rx bol (* space) "#")))
	  (let* ((line (buffer-substring (point) (save-excursion (end-of-line) (point))))
		 (fields (split-string line "\t")))
	    (cond
	     ;;((>= 1 (length line) 0)
	     ;; (message "skipping empty line"))
	     ((= (length fields) 7)
	      (let ((dom (nth 0 fields))
		    (include-subdomains (string= (nth 1 fields) "TRUE"))
		    (path (nth 2 fields))
		    (secure (string= (nth 3 fields) "TRUE"))
		    ;; session cookies (expire time = 0) are supposed
		    ;; to be removed when the browser is closed, but
		    ;; the main point of loading external cookie is to
		    ;; reuse a browser session, so to prevent the
		    ;; cookie from being detected as expired straight
		    ;; away, make it expire a year from now
		    (expires (format-time-string
			      "%d %b %Y %T [GMT]"
			      (let ((s (string-to-number (nth 4 fields))))
				(if (and (zerop s) long-session)
				    (time-add nil (* 365 24 60 60))
				  s))))
		    (key (nth 5 fields))
		    (val (nth 6 fields)))
                (incf n)
		;; Netscape files encode include-subdomains separately; retain
		;; that distinction using a leading dot in the stored domain.
		(let ((domain (if (string-prefix-p "." dom) (substring dom 1) dom)))
		  (url-cookie-store key val expires
				    (if include-subdomains (concat "." domain) domain)
				    path secure))
		))
	     (t
	      (message "ignoring malformed cookie line <%s>" line)))))
	(forward-line))
      (when (< 0 n)
	(setq url-cookies-changed-since-last-save t))
      (message "added %d cookies from file %s" n filename))))

(defun url-cookie-clean-up (&optional secure)
  (let ((var (if secure 'url-cookie-secure-storage 'url-cookie-storage))
	new new-cookies)
    (dolist (cur (symbol-value var))
      (setq new-cookies nil)
      (dolist (cur-cookie (cdr cur))
	(or (not (url-cookie-p cur-cookie))
	    (url-cookie-expired-p cur-cookie)
	    (null (url-cookie-expires cur-cookie))
	    (setq new-cookies (cons cur-cookie new-cookies))))
      (when new-cookies
	(setcdr cur new-cookies)
	(setq new (cons cur new))))
    (set var new)))

(defun url-cookie-write-file (&optional fname)
  (when (and url-cookies-changed-since-last-save
             url-cookie-file)
    (or fname (setq fname (expand-file-name url-cookie-file)))
    (if (condition-case nil
            (progn
              (url-make-private-file fname)
              nil)
          (error t))
        (message "Error accessing cookie file `%s'" fname)
    (url-cookie-clean-up)
    (url-cookie-clean-up t)
    (with-temp-buffer
      (insert ";; Emacs-W3 HTTP cookies file  -*- lexical-binding: t -*-\n"
	      ";; Automatically generated file!!! DO NOT EDIT!!!\n\n"
	      "(setq url-cookie-storage\n '")
      (let ((print-length nil) (print-level nil))
	(pp url-cookie-storage (current-buffer))
	(insert ")\n(setq url-cookie-secure-storage\n '")
	(pp url-cookie-secure-storage (current-buffer)))
      (insert ")\n")
      (generate-lisp-file-trailer fname :inhibit-provide t :autoloads t)
      (setq-local version-control 'never)
      (write-file fname))
    (setq url-cookies-changed-since-last-save nil))))

(defun url-cookie-store (name value &optional expires domain localpart secure)
  "Store a cookie."
  (when (> (length name) 0)
    (let* ((storage-var (if secure 'url-cookie-secure-storage
                          'url-cookie-storage))
           (other-var (if secure 'url-cookie-storage
                        'url-cookie-secure-storage))
           (storage (symbol-value storage-var))
           (other-storage (symbol-value other-var))
           (other-domain (assoc domain other-storage))
	   tmp found-domain)
      ;; Secure is not part of the replacement key.  Remove a matching cookie
      ;; from the other store before adding or updating this one.
      (when other-domain
        (setcdr other-domain
                (cl-delete-if
                 (lambda (cookie)
                   (and (equal localpart (url-cookie-localpart cookie))
                        (equal name (url-cookie-name cookie))))
                 (cdr other-domain)))
        (unless (cdr other-domain)
          (set other-var (delq other-domain other-storage))))
      ;; First, look for a matching domain.
      (if (setq found-domain (assoc domain storage))
          ;; Need to either stick the new cookie in existing domain storage
          ;; or possibly replace an existing cookie if the names match.
          (unless (dolist (cur (setq storage (cdr found-domain)) tmp)
                    (and (equal localpart (url-cookie-localpart cur))
                         (equal name (url-cookie-name cur))
                         (progn
                           (setf (url-cookie-expires cur) expires)
                           (setf (url-cookie-value cur) value)
                           (setf (url-cookie-secure cur) secure)
                           (setq tmp t))))
            ;; New cookie.
            (setcdr found-domain (cons
                                  (url-cookie-create :name name
                                                     :value value
                                                     :expires expires
                                                     :domain domain
                                                     :localpart localpart
                                                     :secure secure)
                                  (cdr found-domain))))
        ;; Need to add a new top-level domain.
        (setq tmp (url-cookie-create :name name
                                     :value value
                                     :expires expires
                                     :domain domain
                                     :localpart localpart
                                     :secure secure))
        (cond (storage
               (setcdr storage (cons (list domain tmp) (cdr storage))))
              (secure
               (setq url-cookie-secure-storage (list (list domain tmp))))
              (t
               (setq url-cookie-storage (list (list domain tmp)))))))))

(defun url-cookie--expires-date-to-time (date)
  "Parse cookie DATE with RFC 6265's year rule."
  (let* ((date-without-time
          (replace-regexp-in-string
           "[0-9]+:[0-9]+:[0-9]+"
           (lambda (match) (make-string (length match) 32))
           date t t))
         (zone-regexp
          "\\(?:^\\|[^[:alnum:]]\\)\\([+-][0-9][0-9]:?[0-9][0-9]\\)\\(?:$\\|[^0-9]\\)")
         (date-without-zone
          (replace-regexp-in-string
           zone-regexp
           (lambda (match) (make-string (length match) 32))
           date-without-time t t))
         (day-token
          (and (string-match
                "\\(?:^\\|[^0-9]\\)\\([0-9][0-9]?\\)\\(?:[^0-9]\\|$\\)"
                date-without-zone)
               (list (match-beginning 1)
                     (match-end 1)
                     (string-to-number (match-string 1 date-without-zone)))))
         (date-with-hyphen-delimiters (copy-sequence date))
         (start 0)
         year-range)
    ;; RFC 6265 processes the day-of-month token before considering a year.
    ;; An out-of-range two-digit day must not be reinterpreted as a year.
    (when (and day-token (not (<= 1 (nth 2 day-token) 31)))
      (error "Invalid cookie date: %s" date))
    ;; Hyphen is an RFC 6265 date-token delimiter.  Leave signed numeric
    ;; timezone offsets intact; they were blanked in DATE-WITHOUT-ZONE.
    (while (string-match "-" date-without-zone start)
      (aset date-with-hyphen-delimiters (match-beginning 0) 32)
      (setq start (match-end 0)))
    (setq start 0)
    ;; RFC 6265 ignores timezone tokens and interprets the parsed fields in
    ;; UTC.  Remove numeric offsets from the output using positions found
    ;; after the time token has been masked.
    (while (string-match zone-regexp date-without-time start)
      (let ((offset (match-beginning 1))
            (offset-end (match-end 1)))
        (while (< offset offset-end)
          (aset date-with-hyphen-delimiters offset 32)
          (setq offset (1+ offset))))
      (setq start (match-end 0)))
    (setq start 0)
    ;; RFC 6265 year tokens may have two to four digits; apply the mapping to
    ;; their numeric value, not their textual width.
    (while (and (not year-range)
                (string-match
                 "\\(?:^\\|[^0-9]\\)\\([0-9][0-9][0-9]?[0-9]?\\)\\(?:$\\|[^0-9]\\)"
                 date-without-zone start))
      (let ((year-start (match-beginning 1))
            (year-end (match-end 1))
            (year (string-to-number (match-string 1 date-without-zone))))
        (cond
         ((and day-token
               (= year-start (nth 0 day-token))
               (= year-end (nth 1 day-token)))
          (setq start year-end))
         ((<= 0 year 69)
          (setq year-range (list year-start year-end year)))
         (t (setq start year-end)))))
    (let ((date-for-time
           (if year-range
               (concat (substring date-with-hyphen-delimiters
                                  0 (nth 0 year-range))
                       (number-to-string (+ 2000 (nth 2 year-range)))
                       (substring date-with-hyphen-delimiters
                                  (nth 1 year-range)))
             date-with-hyphen-delimiters)))
      ;; Ignore any trailing timezone name (for example, PST or GMT).
      (when (string-match "[ \t]+[[:alpha:]]+[ \t]*\\'" date-for-time)
        (setq date-for-time (substring date-for-time 0 (match-beginning 0))))
      (date-to-time (concat date-for-time " GMT")))))

(defun url-cookie-expired-p (cookie)
  "Return non-nil if COOKIE is expired."
  (let ((exp (url-cookie-expires cookie)))
    (and (> (length exp) 0)
	 (condition-case ()
	     (time-less-p (url-cookie--expires-date-to-time exp) nil)
	   (error nil)))))

(defun url-cookie--ipv4-address-p (host)
  "Return non-nil if HOST has dotted-quad IPv4 address syntax."
  (string-match-p "\\`[0-9]+\\(?:\\.[0-9]+\\)\\{3\\}\\'" host))

(defun url-cookie--domain-match-p (host domain &optional include-subdomains)
  ;;Remacs: enforce DNS-label boundaries, ignore trailing-dot domains, and
  ;; preserve host-only semantics.
  "Return non-nil if HOST matches DOMAIN.
When INCLUDE-SUBDOMAINS is non-nil, require a DNS-label boundary."
  (let ((host (downcase host))
	(domain (downcase domain)))
    (or (string= host domain)
	(and include-subdomains
	     ;; RFC 6265 §5.1.3 allows suffix matching only for host names.
	     (not (url-cookie--ipv4-address-p host))
	     ;; A trailing dot makes a Domain attribute invalid (RFC 6265 §4.1.2.3).
	     (not (string-suffix-p "." domain))
	     (string-suffix-p (concat "." domain) host)))))

(defun url-cookie--path-match-p (cookie-path request-path)
  ;;Remacs: a matching path prefix must end at a directory boundary.
  "Return non-nil if COOKIE-PATH matches REQUEST-PATH."
  (let ((path-length (length cookie-path)))
    (and (string-prefix-p cookie-path request-path)
	 (or (= path-length (length request-path))
	     (and (> path-length 0)
		  (eq (aref cookie-path (1- path-length)) ?/))
	     (and (< path-length (length request-path))
		  (eq (aref request-path path-length) ?/))))))

(defun url-cookie--default-path (request-path)
  ;;Remacs: apply RFC 6265's default-path rule to the request path.
  "Return the default cookie path for REQUEST-PATH."
  (if (or (not (stringp request-path))
	  (zerop (length request-path))
	  (not (eq (aref request-path 0) ?/)))
      "/"
    (let ((last-slash (string-match "/[^/]*\\'" request-path)))
      (if (or (null last-slash) (zerop last-slash))
	  "/"
	(substring request-path 0 last-slash)))))

(defun url-cookie-retrieve (host &optional localpart secure)
  "Retrieve all cookies for a specified HOST and LOCALPART."
  (let ((storage (if secure
                     (append url-cookie-secure-storage url-cookie-storage)
                   url-cookie-storage))
        cookies retval localpart-match)
    (dolist (cur storage)
      (let* ((stored-domain (car cur))
             (include-subdomains
              (and (stringp stored-domain)
                   (> (length stored-domain) 0)
                   (eq ?. (aref stored-domain 0))))
             (domain (if include-subdomains
                         (substring stored-domain 1)
                       stored-domain)))
        (setq cookies (cdr cur))
        (when (and (stringp domain)
                   (> (length domain) 0)
                   (url-cookie--domain-match-p
                    host domain include-subdomains))
          ;; The domains match - a possible hit!
          (dolist (cur cookies)
            (and (if (and (stringp
                           (setq localpart-match
                                 (url-cookie-localpart cur)))
                          (stringp localpart))
                     (url-cookie--path-match-p localpart-match localpart)
                   (equal localpart localpart-match))
                 (not (url-cookie-expired-p cur))
                 (setq retval (cons cur retval)))))))
    retval))

(defun url-cookie-generate-header-lines (host localpart secure)
  (let ((cookies (url-cookie-retrieve host localpart secure))
	retval chunk)
    ;; Have to sort this for sending most specific cookies first.
    (setq cookies (and cookies
		       (sort cookies
			     (lambda (x y)
			       (> (length (url-cookie-localpart x))
				  (length (url-cookie-localpart y)))))))
    (dolist (cur cookies)
      (setq chunk (format "%s=%s" (url-cookie-name cur) (url-cookie-value cur))
	    retval (if (and retval url-cookie-multiple-line
			    (< 80 (+ (length retval) (length chunk) 4)))
		       (concat retval "\r\nCookie: " chunk)
		     (if retval
			 (concat retval "; " chunk)
		       (concat "Cookie: " chunk)))))
    (if retval
	(concat retval "\r\n")
      "")))

(defcustom url-cookie-trusted-urls nil
  "A list of regular expressions matching URLs to always accept cookies from."
  :type '(repeat regexp))

(defcustom url-cookie-untrusted-urls nil
  "A list of regular expressions matching URLs to never accept cookies from."
  :type '(repeat regexp))

(defun url-cookie-host-can-set-p (host domain)
  (let ((domain (if (and (> (length domain) 0)
			 (eq ?. (aref domain 0)))
		    (substring domain 1)
		  domain)))
    (cond
     ((and (> (length domain) 0)
	   (url-cookie--domain-match-p host domain))
      ;; Apparently netscape lets cookies be set for the exact host.
      t)
     ((zerop (length domain))
      nil)
     (t
      (and (url-domsuf-cookie-allowed-p domain)
	   (url-cookie--domain-match-p host domain t))))))

(defun url-cookie-handle-set-cookie (str)
  (setq url-cookies-changed-since-last-save t)
  (let* ((args (url-parse-args str t))
	 (case-fold-search t)
	 (secure (and (assoc-string "secure" args t) t))
	 (domain-attribute
	  (let* ((attribute (assoc-string "domain" args t))
		 (value (cdr-safe attribute)))
	    ;;Remacs: RFC 6265 ignores empty and trailing-dot Domain attributes,
	    ;; leaving the cookie host-only.
	    (and attribute (stringp value) (> (length value) 0)
		 (not (eq ?. (aref value (1- (length value)))))
		 attribute)))
	 (domain (if domain-attribute
		      (cdr-safe domain-attribute)
		    (url-host url-current-object)))
	 ;; A leading dot records a Domain attribute; no dot means host-only.
	 (stored-domain (if (and domain-attribute
			  (> (length domain) 0)
			  (not (eq ?. (aref domain 0))))
		     (concat "." domain)
		   domain))
	 (current-url (url-view-url t))
	 (trusted url-cookie-trusted-urls)
	 (untrusted url-cookie-untrusted-urls)
	 (max-age (cdr-safe (assoc-string "max-age" args t)))
	 (path-attribute (cdr-safe (assoc-string "path" args t)))
	 (localpart
	  (if (and (stringp path-attribute)
		   (> (length path-attribute) 0)
		   (eq (aref path-attribute 0) ?/))
	      path-attribute
	    (url-cookie--default-path
	     (car (url-path-and-query url-current-object)))))
	 (expires nil))
    (if (and max-age (string-match "\\`-?[0-9]+\\'" max-age))
	(setq expires (ignore-errors
                        (format-time-string "%a %b %d %H:%M:%S %Y GMT"
					    (time-add nil (read max-age))
					    t)))
      (setq expires (cdr-safe (assoc-string "expires" args t))))
    (while (consp trusted)
      (if (string-match (car trusted) current-url)
	  (setq trusted (- (match-end 0) (match-beginning 0)))
	(pop trusted)))
    (while (consp untrusted)
      (if (string-match (car untrusted) current-url)
	  (setq untrusted (- (match-end 0) (match-beginning 0)))
	(pop untrusted)))
    (and trusted untrusted
	 ;; Choose the more specific match.
	 (if (> trusted untrusted) (setq untrusted nil) (setq trusted nil)))
    (cond
     (untrusted
      ;; The site was explicitly marked as untrusted by the user.
      nil)
     ((or (eq url-privacy-level 'paranoid)
	  (and (listp url-privacy-level) (memq 'cookies url-privacy-level)))
      ;; User never wants cookies.
      nil)
     ((and url-cookie-confirmation
	   (not trusted)
	   (save-window-excursion
	     (with-output-to-temp-buffer "*Cookie Warning*"
	       (princ (format "%s=\"%s\"\n" (caar args) (cdar args)))
	       (dolist (x (cdr args))
		 (princ (format "  %s=\"%s\"\n" (car x) (cdr x)))))
	     (prog1
		 (not (funcall url-confirmation-func
			       (format "Allow %s to set these cookies? "
				       (url-host url-current-object))))
	       (if (get-buffer "*Cookie Warning*")
		   (kill-buffer "*Cookie Warning*")))))
      ;; User wants to be asked, and declined.
      nil)
     ((url-cookie-host-can-set-p (url-host url-current-object) domain)
      ;; Cookie is accepted by the user, and passes our security checks.
      (url-cookie-store (caar args) (cdar args)
			expires stored-domain localpart secure))
     (t
      (url-lazy-message "%s tried to set a cookie for domain %s - rejected."
			(url-host url-current-object) domain)))))

(defvar url-cookie-timer nil)

(defcustom url-cookie-save-interval 3600
  "If non-nil, the number of seconds between automatic saves of cookies.
Default is 1 hour; set to nil to disable automatic saving of cookies.
Note that if you change this variable outside of the `customize'
interface after `url-do-setup' has been run, you need to run
the `url-cookie-setup-save-timer' function manually."
  :set (lambda (var val)
         (set-default var val)
         (if (bound-and-true-p url-setup-done)
             (url-cookie-setup-save-timer)))
  :type '(choice (const :tag "Disable automatic saving of cookies" :value nil)
                 (natnum :tag "Interval in seconds for auto-saving cookies")))

(defun url-cookie-setup-save-timer ()
  "Reset the cookie saver timer."
  (interactive)
  (ignore-errors (cancel-timer url-cookie-timer))
  (setq url-cookie-timer nil)
  (if (natnump url-cookie-save-interval)
      (setq url-cookie-timer (run-at-time url-cookie-save-interval
					  url-cookie-save-interval
					  #'url-cookie-write-file))))

(defun url-cookie-delete-cookies (&optional regexp keep)
  "Delete all cookies from the cookie store where the domain matches REGEXP.
If REGEXP is nil, all cookies are deleted.  If KEEP is non-nil,
instead delete all cookies that do not match REGEXP."
  (dolist (variable '(url-cookie-secure-storage url-cookie-storage))
    (let ((cookies (symbol-value variable)))
      (dolist (elem cookies)
        (when (or (and (null keep)
                       (or (null regexp)
                           (string-match regexp (car elem))))
                  (and keep
                       regexp
                       (not (string-match regexp (car elem)))))
          (setq cookies (delq elem cookies))))
      (set variable cookies)))
  (setq url-cookies-changed-since-last-save t)
  (url-cookie-write-file))

;;; Mode for listing and editing cookies.

(defvar url-cookie--deleted-cookies nil)

(defun url-cookie-list ()
  "Display a buffer listing the current URL cookies, if there are any.
Use \\<url-cookie-mode-map>\\[url-cookie-delete] to remove cookies."
  (interactive)
  (unless (or url-cookie-secure-storage
              url-cookie-storage)
    (error "No cookies are defined"))

  (pop-to-buffer "*url cookies*")
  (url-cookie-mode)
  (url-cookie--generate-buffer)
  (goto-char (point-min)))

(defun url-cookie--generate-buffer ()
  (let ((inhibit-read-only t)
	(domains (sort
		  (copy-sequence
		   (append url-cookie-secure-storage
			   url-cookie-storage))
		  (lambda (e1 e2)
		    (string< (car e1) (car e2)))))
	(domain-length 0)
	start name format domain)
    (erase-buffer)
    (dolist (elem domains)
      (setq domain-length (max domain-length (length (car elem)))))
    (setq format (format "%%-%ds %%-20s %%s" domain-length)
	  header-line-format
	  (concat " " (format format "Domain" "Name" "Value")))
    (dolist (elem domains)
      (setq domain (car elem))
      (dolist (cookie (sort (copy-sequence (cdr elem))
			    (lambda (c1 c2)
			      (string< (url-cookie-name c1)
				       (url-cookie-name c2)))))
        (setq start (point)
	      name (url-cookie-name cookie))
        (when (> (length name) 20)
	  (setq name (substring name 0 20)))
        (insert (format format domain name
		        (url-cookie-value cookie))
	        "\n")
        (setq domain "")
        (put-text-property start (1+ start) 'url-cookie cookie)))))

(defun url-cookie-delete ()
  "Delete the cookie on the current line."
  (interactive)
  (let ((cookie (get-text-property (line-beginning-position) 'url-cookie))
	(inhibit-read-only t)
	variable)
    (unless cookie
      (error "No cookie on the current line"))
    (setq variable (if (url-cookie-secure cookie)
		       'url-cookie-secure-storage
		     'url-cookie-storage))
    (let* ((list (symbol-value variable))
	   (elem (assoc (url-cookie-domain cookie) list)))
      (setq elem (delq cookie elem))
      (when (zerop (length (cdr elem)))
	(setq list (delq elem list)))
      (set variable list))
    (setq url-cookies-changed-since-last-save t)
    (url-cookie-write-file)
    (delete-region (line-beginning-position)
		   (progn
		     (forward-line 1)
		     (point)))
    (let ((point (point)))
      (erase-buffer)
      (url-cookie--generate-buffer)
      (goto-char point))
    (push cookie url-cookie--deleted-cookies)))

(defun url-cookie-undo ()
  "Undo deletion of a cookie."
  (interactive)
  (unless url-cookie--deleted-cookies
    (error "No cookie deletions to undo"))
  (let* ((cookie (pop url-cookie--deleted-cookies))
         (variable (if (url-cookie-secure cookie)
		       'url-cookie-secure-storage
		     'url-cookie-storage))
         (list (symbol-value variable))
	 (elem (assoc (url-cookie-domain cookie) list)))
    (if elem
        (nconc elem (list cookie))
      (setq elem (list (url-cookie-domain cookie) cookie))
      (set variable (cons elem list)))
    (setq url-cookies-changed-since-last-save t)
    (url-cookie-write-file)
    (let ((point (point))
          (inhibit-read-only t))
      (erase-buffer)
      (url-cookie--generate-buffer)
      (goto-char point))))

(defvar-keymap url-cookie-mode-map
  "<delete>" #'url-cookie-delete
  "C-k"      #'url-cookie-delete
  "C-_"      #'url-cookie-undo)

(define-derived-mode url-cookie-mode special-mode "URL Cookie"
  "Mode for listing cookies.

\\{url-cookie-mode-map}"
  (buffer-disable-undo)
  (setq buffer-read-only t
	truncate-lines t))

(provide 'url-cookie)

;;; url-cookie.el ends here
