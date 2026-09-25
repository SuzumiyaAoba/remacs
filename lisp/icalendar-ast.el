;;; icalendar-ast.el --- Syntax trees for iCalendar  -*- lexical-binding: t; -*-

;; Copyright (C) 2024 Free Software Foundation, Inc.

;; Author: Richard Lawrence <rwl@recursewithless.net>
;; Created: October 2024
;; Keywords: calendar
;; Human-Keywords: calendar, iCalendar
;; Package: icalendar

;; This file is part of GNU Emacs.

;; This file is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; This file is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with this file.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; For an overview of the iCalendar library, see icalendar-shortdoc.el.

;; This file defines the abstract syntax tree representation for
;; iCalendar data.  The AST is based on `org-element-ast' (which see;
;; that feature will eventually be renamed and moved out of the Org tree
;; into the main tree).

;; This file contains low-level functions for constructing and
;; manipulating the AST, most of which are minimal wrappers around the
;; functions provided by `org-element-ast'.  This low-level API is
;; primarily used by `icalendar-parser'.  It also contains a higher-level
;; API for constructing AST nodes in Lisp code.  Finally, it defines
;; functions for validating AST nodes.

;; There are three main pieces of data in an AST node: its type, its
;; value, and its child nodes.  Nodes which represent iCalendar
;; components have no values; they are simply containers for their
;; children.  Nodes which represent data of the base iCalendar data
;; types have no children; they are the leaf nodes in the syntax tree.
;; The main low-level accessors for these data in AST nodes are:
;;
;;   `icalendar-ast-node-type'
;;   `icalendar-ast-node-value'
;;   `icalendar-ast-node-children'
;;   `icalendar-ast-node-children-of'
;;   `icalendar-ast-node-first-child-of'

;; To construct AST nodes in Lisp code, see especially the high-level macros:
;;
;;   `icalendar-make-vcalendar'
;;   `icalendar-make-vtimezone'
;;   `icalendar-make-vevent'
;;   `icalendar-make-vtodo'
;;   `icalendar-make-vjournal'
;;   `icalendar-make-property'
;;   `icalendar-make-param'
;;
;; These macros wrap the macro `icalendar-make-node-from-templates',
;; which allows writing iCalendar syntax tree nodes as Lisp templates.

;; Constructing nodes with these macros automatically validates them
;; with the function `icalendar-ast-node-valid-p', which signals an
;; `icalendar-validation-error' if the node is not valid according to
;; RFC5545.


;;; Code:
(eval-when-compile (require 'icalendar-macs))
(require 'icalendar)
(require 'org-element-ast)
(require 'cl-lib)

;;; Type symbols and metadata

;; All nodes in the syntax tree have a type symbol as their first element.
;; We use the following symbol properties (all prefixed with 'icalendar-')
;; to associate type symbols with various important data about the type:
;;
;; is-type - t (marks this symbol as an icalendar type)
;; is-value, is-param, is-property, or is-component - t
;;   (specifies what sort of value this type represents)
;; list-sep - for property and parameters types, a string (typically
;;   "," or ";") which separates individual printed values, if the
;;   type allows lists of values.  If this is non-nil, syntax nodes of
;;   this type should always have a list of values in their VALUE
;;   field (even if there is only one value)
;; matcher - a function to match this type.  This function matches the
;;   regular expression defined under the type's name; it is used to provide
;;   syntax highlighting in `icalendar-mode'
;; begin-rx, end-rx - for component-types, an `rx' regular expression which
;;   matches the BEGIN and END lines that form its boundaries
;; value-rx - an `rx' regular expression which matches individual values
;;   of this type, with no consideration for quoting or lists of values.
;;   (For value types, this is just a synonym for the rx definition
;;   under the type's symbol)
;; values-rx - for types that accept lists of values, an `rx' regular
;;   expression which matches the whole list (including quotes, if required)
;; full-value-rx - for property and parameter types, an `rx' regular
;;   expression which matches a valid value expression in group 2, or
;;   an invalid value in group 3
;; value-reader - for value types, a function which creates syntax
;;   nodes of this type given a string representing their value
;; value-printer - for value types, a function to print individual
;;   values of this type.  It accepts a value and returns its string
;;   representation.
;; default-value - for property and parameter types, a string
;;   representing a default value for nodes of this type.  This is the
;;   value assumed when no node of this type is present in the
;;   relevant part of the syntax tree.
;; substitute-value - for parameter types, a string representing a value
;;   which will be substituted at parse times for unrecognized values.
;;   (This is normally the same as default-value, but differs from it
;;   in at least one case in RFC5545, thus it is stored separately.)
;; default-type - for property types which can accept values of multiple
;;   types, this is the default type when no type for the value is
;;   specified in the parameters.  Any type of value other than this
;;   one requires a VALUE=... parameter when the property is read or printed.
;; other-types - for property types which can accept values of multiple types,
;;   this is a list of other types that the property can accept.
;; value-type - for param types, this is the value type which the parameter
;;   can accept.
;; child-spec - for property and component types, a plist describing the
;;   required and optional child nodes.  See `icalendar-define-property' and
;;   `icalendar-define-component' for details.
;; other-validator - a function to perform type-specific validation
;;   for nodes of this type.  If present, this function will be called
;;   by `icalendar-ast-node-valid-p' during validation.
;; type-documentation - a string documenting the type.  This documentation is
;;   printed in the help buffer when `describe-symbol' is called on TYPE.
;; link - a hyperlink to the documentation of the type in the relevant standard

(defun icalendar-type-symbol-p (symbol)
  "Return non-nil if SYMBOL is an iCalendar type symbol.

This function only checks that SYMBOL has been marked as a type;
it returns t for value types defined by `icalendar-define-type',
but also e.g. for types defined by `icalendar-define-param' and
`icalendar-define-property'.  To check that SYMBOL names a value
type for property or parameter values, see
`icalendar-value-type-symbol-p' and
`icalendar-printable-value-type-symbol-p'."
  (and (symbolp symbol)
       (get symbol 'icalendar-is-type)))

(defun icalendar-value-type-symbol-p (symbol)
  "Return non-nil if SYMBOL is a type symbol for a value type.

This means that SYMBOL must both satisfy `icalendar-type-symbol-p' and
have the property `icalendar-is-value'.  It does not require the type to
be associated with a print name in `icalendar-value-types'; for that see
`icalendar-printable-value-type-symbol-p'."
  (and (icalendar-type-symbol-p symbol)
       (get symbol 'icalendar-is-value)))

(defun icalendar-expects-list-of-values-p (type)
  "Return non-nil if TYPE expects a list of values.

This is never t for value types or component types.  For property and
parameter types defined with `icalendar-define-param' and
`icalendar-define-property', it is true if the :list-sep argument was
specified in the definition."
  (and (icalendar-type-symbol-p type)
       (get type 'icalendar-list-sep)))

(defun icalendar-param-type-symbol-p (type)
  "Return non-nil if TYPE is a type symbol for an iCalendar parameter."
  (and (icalendar-type-symbol-p type)
       (get type 'icalendar-is-param)))

(defun icalendar-property-type-symbol-p (type)
  "Return non-nil if TYPE is a type symbol for an iCalendar property."
  (and (icalendar-type-symbol-p type)
       (get type 'icalendar-is-property)))

(defun icalendar-component-type-symbol-p (type)
  "Return non-nil if TYPE is a type symbol for an iCalendar component."
  (and (icalendar-type-symbol-p type)
       (get type 'icalendar-is-component)))

;; TODO: we could define other accessors here for the other metadata
;; properties, but at the moment I see no advantage to this; they would
;; all just be long-winded wrappers around `get'.


;; The basic, low-level API for the AST, mostly intended for use by
;; `icalendar-parser'.  These functions are mostly aliases and simple
;; wrappers around functions provided by `org-element-ast', which does
;; the heavy lifting.
(defalias 'icalendar-ast-node-type #'org-element-type)

(defsubst icalendar-ast-node-value (node)
  "Return the value of iCalendar syntax node NODE.
In component nodes, this is nil.  Otherwise, it is a syntax node
representing an iCalendar (property or parameter) value."
  (org-element-property :value node))

(defalias 'icalendar-ast-node-children #'org-element-contents)

;; TODO: probably don't want &rest form for this
(defalias 'icalendar-ast-node-set-children #'org-element-set-contents)

(defalias 'icalendar-ast-node-adopt-children #'org-element-adopt-elements)

(defalias 'icalendar-ast-node-meta-get #'org-element-property)

(defalias 'icalendar-ast-node-meta-set #'org-element-put-property)

(defun icalendar-ast-node-set-type (node type)
  "Set the type of iCalendar syntax node NODE to TYPE.

This function is probably not what you want!  It directly modifies the
type of NODE in-place, which could make the node invalid if its value or
children do not match the new TYPE.  If you do not know in advance that
the data in NODE is compatible with the new TYPE, it is better to
construct a new syntax node."
  (setcar node type))

(defun icalendar-ast-node-set-value (node value)
  "Set the value of iCalendar syntax node NODE to VALUE."
  (icalendar-ast-node-meta-set node :value value))

(defun icalendar-make-ast-node (type props &optional children)
  "Construct a syntax node of TYPE with meta-properties PROPS and CHILDREN.

This is a low-level constructor.  If you are constructing iCalendar
syntax nodes directly in Lisp code, consider using one of the
higher-level macros based on `icalendar-make-node-from-templates'
instead, which expand to calls to this function but also perform type
checking and validation.

TYPE should be an iCalendar type symbol.  CHILDREN, if given, should be
a list of syntax nodes.  In property nodes, these should be the
parameters of the property.  In component nodes, these should be the
properties or subcomponents of the component.  CHILDREN should otherwise
be nil.

PROPS should be a plist with any of the following keywords:

:value - in value nodes, this should be the Elisp value parsed from a
  property or parameter's value string.  In parameter and property nodes,
  this should be a value node or list of value nodes.  In component
  nodes, it should not be present.
:buffer - buffer from which VALUE was parsed
:begin - position at which this node begins in BUFFER
:end - position at which this node ends in BUFFER
:value-begin - position at which VALUE begins in BUFFER
:value-end - position at which VALUE ends in BUFFER
:original-value - a string containing the original, uninterpreted value
  of the node.  This can differ from (a string represented by) VALUE
  if e.g. a default VALUE was substituted for an unrecognized but
  syntactically correct value.
:original-name - a string containing the original, uninterpreted name
  of the parameter, property or component this node represents.
  This can differ from (a string representing) TYPE
  if e.g. a default TYPE was substituted for an unrecognized but
  syntactically correct one."
  ;; automatically mark :value as a "secondary property" for org-element-ast
  (let ((full-props (if (plist-member props :value)
                        (plist-put props :secondary (list :value))
                      props)))
    (apply #'org-element-create type full-props children)))

(defun icalendar-ast-node-p (val)
  "Return non-nil if VAL is an iCalendar syntax node."
  (and (listp val)
       (length> val 1)
       (icalendar-type-symbol-p (icalendar-ast-node-type val))
       (plistp (cadr val))
       (listp (icalendar-ast-node-children val))))

(defun icalendar-param-node-p (node)
  "Return non-nil if NODE is a syntax node whose type is a parameter type."
  (and (icalendar-ast-node-p node)
       (icalendar-param-type-symbol-p (icalendar-ast-node-type node))))

(defun icalendar-property-node-p (node)
  "Return non-nil if NODE is a syntax node whose type is a property type."
  (and (icalendar-ast-node-p node)
       (icalendar-property-type-symbol-p (icalendar-ast-node-type node))))

(defun icalendar-component-node-p (node)
  "Return non-nil if NODE is a syntax node whose type is a component type."
  (and (icalendar-ast-node-p node)
       (icalendar-component-type-symbol-p (icalendar-ast-node-type node))))

(defun icalendar-ast-node-first-child-of (type node)
  "Return the first child of NODE of type TYPE, or nil."
  (assq type (icalendar-ast-node-children node)))

(defun icalendar-ast-node-children-of (type node)
  "Return a list of all the children of NODE of type TYPE."
  (let (tchildren)
    (dolist (c (icalendar-ast-node-children node))
      (when (eq type (icalendar-ast-node-type c))
        (push c tchildren)))
    (nreverse tchildren)))


;; A high-level API for constructing iCalendar syntax nodes in Lisp code:
(defun icalendar-type-of (value &optional types)
  "Find the iCalendar type symbol for the type to which VALUE belongs.

TYPES, if specified, should be a list of type symbols to check.
TYPES defaults to all type symbols listed in `icalendar-value-types'."
  (require 'icalendar-parser) ; for icalendar-value-types, icalendar-list-of-p
  (declare-function icalendar-list-of-p "icalendar-parser")
  (catch 'found
    (when (icalendar-ast-node-p value)
      (throw 'found (icalendar-ast-node-type value)))
    ;; FIXME:  the warning here is spurious, given that icalendar-parser
    ;; is require'd above:
    (with-suppressed-warnings ((free-vars icalendar-value-types))
      (dolist (type (or types (mapcar #'cdr icalendar-value-types)))
        (if (icalendar-expects-list-of-values-p type)
            (when (icalendar-list-of-p value type)
              (throw 'found type))
          (when (cl-typep value type)
            (throw 'found type)))))))

;; A more flexible constructor for value nodes which can choose the
;; correct type from a list.  This helps keep templates succinct and easy
;; to use in `icalendar-make-node-from-templates', and related macros
;; below.
(defun icalendar-make-value-node-of (type value)
  "Make an iCalendar syntax node of type TYPE containing VALUE as its value.

TYPE should be a symbol for an iCalendar value type, and VALUE should be
a value of that type.  If TYPE is the symbol \\='plain-text, VALUE should
be a string, and in that case VALUE is returned as-is.

TYPE may also be a list of type symbols; in that case, the first type in
the list which VALUE satisfies is used as the returned node's type.  If
the list is nil, VALUE will be checked against all types in
`icalendar-value-types'.

If VALUE is nil, and `icalendar-boolean' is not (in) TYPE, nil is
returned.  Otherwise, a \\='wrong-type-argument error is signaled if
VALUE does not satisfy (any type in) TYPE."
  (require 'icalendar-parser) ; for `icalendar-list-of-p'
  (cond
   ((and (null value)
         (not (if (listp type) (memq 'icalendar-boolean type)
                (eq 'icalendar-boolean type))))
    ;; Instead of signaling an error, we just return nil in this case.
    ;; This allows the `icalendar-make-*' macros higher up the stack to
    ;; filter out templates that evaluate to nil at run time:
    nil)
   ((eq type 'plain-text)
    (unless (stringp value)
      (signal 'wrong-type-argument (list 'stringp value)))
    value)
   ((symbolp type)
    (unless (icalendar-value-type-symbol-p type)
      (signal 'wrong-type-argument (list 'icalendar-value-type-symbol-p type)))
    (if (icalendar-expects-list-of-values-p type)
        (unless (icalendar-list-of-p value type)
          (signal 'wrong-type-argument (list `(list-of ,type) value)))
      (unless (cl-typep value type)
        (signal 'wrong-type-argument (list type value)))
      (icalendar-make-ast-node type (list :value value))))
   ((listp type)
    ;; N.B. nil is allowed; in that case, `icalendar-type-of' will check all
    ;; types in `icalendar-value-types':
    (let ((the-type (icalendar-type-of value type)))
      (if the-type
          (icalendar-make-ast-node the-type (list :value value))
        (signal 'wrong-type-argument
                (list (if (length> type 1) (cons 'or type) (car type))
                      value)))))
   (t (signal 'wrong-type-argument (list '(or symbolp listp) type)))))

(defun icalendar--make-param--list (type value-type raw-values)
  "Make a param node of TYPE with list of values RAW-VALUES of type VALUE-TYPE."
  (let ((value (if (seq-every-p #'icalendar-ast-node-p raw-values)
                   raw-values
                 (mapcar
                  (lambda (c)
                    (icalendar-make-value-node-of value-type c))
                  raw-values))))
    (when value
      (icalendar-ast-node-valid-p
       (icalendar-make-ast-node
        type
        (list :value value))))))

(defun icalendar--make-param--nonlist (type value-type raw-value)
  "Make a param node of TYPE with value RAW-VALUE of type VALUE-TYPE."
  (let ((value (if (icalendar-ast-node-p raw-value)
                   raw-value
                 (icalendar-make-value-node-of value-type raw-value))))
    (when value
      (icalendar-ast-node-valid-p
       (icalendar-make-ast-node
        type
        (list :value value))))))

(defmacro icalendar-make-param (type value)
  "Construct an iCalendar parameter node of TYPE with value VALUE.

TYPE should be an iCalendar type symbol satisfying
`icalendar-param-type-symbol-p'; it should not be quoted.

VALUE should evaluate to a value appropriate for TYPE.  In particular, if
TYPE expects a list of values (see `icalendar-expects-list-p'), VALUE
should be such a list.  If necessary, the value(s) in VALUE will be
wrapped in syntax nodes indicating their type.

For example,

  (icalendar-make-param icalendar-deltoparam
    (list \"mailto:minionA@example.com\" \"mailto:minionB@example.com\"))

will return an `icalendar-deltoparam' node whose value is a list of
`icalendar-cal-address' nodes containing the two addresses.

The resulting syntax node is checked for validity by
`icalendar-ast-node-valid-p' before it is returned."
  (declare (debug (symbolp form)))
  ;; TODO: support `icalendar-otherparam'
  (unless (icalendar-param-type-symbol-p type)
    (error "Not an iCalendar param type: %s" type))
  (let ((value-type (or (get type 'icalendar-value-type) 'plain-text)))
    (if (icalendar-expects-list-of-values-p type)
        `(icalendar--make-param--list ',type ',value-type ,value)
      `(icalendar--make-param--nonlist ',type ',value-type ,value))))

(defun icalendar--make-property--list (type value-types raw-values &optional params)
  "Make a property node of TYPE with list of values RAW-VALUES.
VALUE-TYPES should be a list of value types that TYPE accepts.
PARAMS, if given, should be a list of parameter nodes."
  (require 'icalendar-parser) ; for `icalendar-maybe-add-value-param'
  (declare-function icalendar-maybe-add-value-param "icalendar-parser")

  (let ((value (if (seq-every-p #'icalendar-ast-node-p raw-values)
                   raw-values
                 (mapcar
                  (lambda (c) (icalendar-make-value-node-of value-types c))
                  raw-values))))
    (when value
      (icalendar-ast-node-valid-p
       (icalendar-maybe-add-value-param
        (icalendar-make-ast-node type (list :value value) params))))))

(defun icalendar--make-property--nonlist (type value-types raw-value &optional params)
  "Make a property node of TYPE with value RAW-VALUE.
VALUE-TYPES should be a list of value types that TYPE accepts.
PARAMS, if given, should be a list of parameter nodes."
  (require 'icalendar-parser) ; for `icalendar-maybe-add-value-param'
  (declare-function icalendar-maybe-add-value-param "icalendar-parser")

  (let ((value (if (icalendar-ast-node-p raw-value)
                   raw-value
                 (icalendar-make-value-node-of value-types raw-value))))
    (when value
      (icalendar-ast-node-valid-p
       (icalendar-maybe-add-value-param
        (icalendar-make-ast-node type (list :value value) params))))))

(defmacro icalendar-make-property (type value &rest param-templates)
  "Construct an iCalendar property node of TYPE with value VALUE.

TYPE should be an iCalendar type symbol satisfying
`icalendar-property-type-symbol-p'; it should not be quoted.

VALUE should evaluate to a value appropriate for TYPE.  In particular,
if TYPE expects a list of values (see
`icalendar-expects-list-of-values-p'), VALUE should be such a list.  If
necessary, the value(s) in VALUE will be wrapped in syntax nodes
indicating their type.  If VALUE is not of the default value type for
TYPE, an `icalendar-valuetypeparam' will automatically be added to
PARAM-TEMPLATES.

Each element of PARAM-TEMPLATES should represent a parameter node; see
`icalendar-make-node-from-templates' for the format of such templates.
A template can also have the form (@ L), where L evaluates to a list of
parameter nodes to be added to the component.

PARAM-TEMPLATES which evaluate to nil are removed when the property node
is constructed.

For example,

  (icalendar-make-property icalendar-rdate (list \\='(2 1 2025) \\='(3 1 2025)))

will return an `icalendar-rdate' node whose value is a list of
`icalendar-date' nodes containing the dates above as their values.

The resulting syntax node is checked for validity by
`icalendar-ast-node-valid-p' before it is returned."
  ;; TODO: support `icalendar-other-property', maybe like
  ;; (icalendar-other-property "X-NAME" value ...)
  (declare (debug (symbolp form &rest form))
           (indent 2))
  (unless (icalendar-property-type-symbol-p type)
    (error "Not an iCalendar property type: %s" type))
  (let ((value-types (cons (get type 'icalendar-default-type)
                           (get type 'icalendar-other-types)))
        params-expr children lists-of-children)
    (dolist (c param-templates)
      (cond ((and (listp c) (icalendar-type-symbol-p (car c)))
             ;; c is a template for a child node, so it should be
             ;; recursively expanded:
             (push (cons 'icalendar-make-node-from-templates c)
                   children))
            ((and (listp c) (eq '@ (car c)))
             ;; c is a template (@ L) where L evaluates to a list of children:
             (push (cadr c) lists-of-children))
            (t
             ;; otherwise, just pass c through as is; this allows
             ;; interleaving templates with other expressions that
             ;; evaluate to syntax nodes:
             (push c children))))
    (when (or children lists-of-children)
      (setq params-expr
            `(seq-filter #'identity
                         (append (list ,@children) ,@lists-of-children))))

    (if (icalendar-expects-list-of-values-p type)
        `(icalendar--make-property--list ',type ',value-types ,value ,params-expr)
      `(icalendar--make-property--nonlist ',type ',value-types ,value ,params-expr))))

(defmacro icalendar-make-component (type &rest templates)
  "Construct an iCalendar component node of TYPE from TEMPLATES.

TYPE should be an iCalendar type symbol satisfying
`icalendar-component-type-symbol-p'; it should not be quoted.

Each expression in TEMPLATES should represent a child node of the
component; see `icalendar-make-node-from-templates' for the format of
such TEMPLATES.  A template can also have the form (@ L), where L
evaluates to a list of child nodes to be added to the component.

Any value in TEMPLATES that evaluates to nil will be removed before the
component node is constructed.

If TYPE is `icalendar-vevent', `icalendar-vtodo', `icalendar-vjournal',
or `icalendar-vfreebusy', the properties `icalendar-dtstamp' and
`icalendar-uid' will be automatically provided, if they are absent in
TEMPLATES.  Likewise, if TYPE is `icalendar-vcalendar', the properties
`icalendar-prodid', `icalendar-version', and `icalendar-calscale' will
be automatically provided if absent.

For example,

  (icalendar-make-component icalendar-vevent
     (icalendar-summary \"Party\")
     (icalendar-location \"Robot House\")
     (@ list-of-other-properties))

will return an `icalendar-vevent' node containing the provided
properties as well as `icalendar-dtstamp' and `icalendar-uid'
properties.

The resulting syntax node is checked for validity by
`icalendar-ast-node-valid-p' before it is returned."
  (declare (debug (symbolp &rest form))
           (indent 1))
  ;; TODO: support `icalendar-other-component', maybe like
  ;; (icalendar-other-component (:x-name "X-NAME") templates ...)
  (unless (icalendar-component-type-symbol-p type)
    (error "Not an iCalendar component type: %s" type))
  ;; Add templates for required properties automatically if we can:
  (when (memq type '(icalendar-vevent icalendar-vtodo icalendar-vjournal icalendar-vfreebusy))
    (unless (assq 'icalendar-dtstamp templates)
      (push
       '(icalendar-make-property icalendar-dtstamp
            (let ((stamp (decode-time nil t)))
              ;; Ensure we return UTC, not just :zone 0:
              (setf (decoded-time-zone stamp) t)
              stamp))
       templates))
    (unless (assq 'icalendar-uid templates)
      (push `(icalendar-uid ,(icalendar-make-uid templates))
            templates)))
  (when (eq type 'icalendar-vcalendar)
    (unless (assq 'icalendar-prodid templates)
      (push `(icalendar-prodid ,icalendar-vcalendar-prodid)
            templates))
    (unless (assq 'icalendar-version templates)
      (push `(icalendar-version ,icalendar-vcalendar-version)
            templates))
    (unless (assq 'icalendar-calscale templates)
      (push '(icalendar-calscale "GREGORIAN")
            templates)))
  (when (null templates)
    (error "At least one template is required"))

  (let (children lists-of-children)
    (dolist (c templates)
      (cond ((and (listp c) (icalendar-type-symbol-p (car c)))
             ;; c is a template for a child node, so it should be
             ;; recursively expanded:
             (push (cons 'icalendar-make-node-from-templates c)
                   children))
            ((and (listp c) (eq '@ (car c)))
             ;; c is a template (@ L) where L evaluates to a list of children:
             (push (cadr c) lists-of-children))
            (t
             ;; otherwise, just pass c through as is; this allows
             ;; interleaving templates with other expressions that
             ;; evaluate to syntax nodes:
             (push c children))))
    (setq children (nreverse children)
          lists-of-children (nreverse lists-of-children))
    (when (or children lists-of-children)
      `(icalendar-ast-node-valid-p
        (icalendar-make-ast-node
         (quote ,type)
         nil
         (seq-filter #'identity
                     (append (list ,@children) ,@lists-of-children)))))))

;; TODO: allow disabling the validity check??
(defmacro icalendar-make-node-from-templates (type &rest templates)
  "Construct an iCalendar syntax node of TYPE from TEMPLATES.

TYPE should be an iCalendar type symbol; it should not be quoted.  This
macro (and the derived macros `icalendar-make-vcalendar',
`icalendar-make-vevent', `icalendar-make-vtodo',
`icalendar-make-vjournal', `icalendar-make-vfreebusy',
`icalendar-make-valarm', `icalendar-make-vtimezone',
`icalendar-make-standard', and `icalendar-make-daylight') makes it easy
to write iCalendar syntax nodes of TYPE as Lisp code.

Each expression in TEMPLATES represents a child node of the constructed
node.  It must either evaluate to such a node, or it must have one of
the following forms:

\(VALUE-TYPE VALUE) - constructs a node of VALUE-TYPE containing the
  value VALUE.

\(PARAM-TYPE VALUE) - constructs a parameter node of PARAM-TYPE
  containing the VALUE.

\(PROPERTY-TYPE VALUE [PARAM ...]) - constructs a property node of
  PROPERTY-TYPE containing the value VALUE and PARAMs as child
  nodes.  Each PARAM should be a template (PARAM-TYPE VALUE), as above,
  or any other expression that evaluates to a parameter node.

\(COMPONENT-TYPE CHILD [CHILD ...]) - constructs a component node of
  COMPONENT-TYPE with given child nodes.  Each CHILD should either be
  a template for a property (as above), a template for a
  sub-component (of the same form), or any other expression that
  evaluates to an iCalendar syntax node.

If TYPE is an iCalendar component or property type, a TEMPLATE can also
have the form (@ L), where L evaluates to a list of child nodes to be
added to the component or property node.

For example, an iCalendar VEVENT could be written like this:

  (icalendar-make-node-from-templates icalendar-vevent
    (icalendar-uid \"some-unique-id\")
    (icalendar-summary \"Party\")
    (icalendar-location \"Robot House\")
    (icalendar-organizer \"mailto:bender@mars.edu\")
    (icalendar-attendee  \"mailto:philip.j.fry@mars.edu\"
      (icalendar-partstatparam \"ACCEPTED\"))
    (icalendar-attendee  \"mailto:gunther@mars.edu\"
      (icalendar-partstatparam \"DECLINED\"))
    (icalendar-categories (list \"MISCHIEF\" \"DOUBLE SECRET PROBATION\"))
    (icalendar-dtstart (icalendar-make-date-time :year 3003 :month 3 :day 13
                                                 :hour 22 :minute 0 :second 0)
       (icalendar-tzidparam \"Mars/University_Time\")))

Before the constructed node is returned, it is validated by
`icalendar-ast-node-valid-p'."
  (declare (debug (symbolp &rest form))
           (indent 1))
  (cond
   ((not (icalendar-type-symbol-p type))
    (error "Not an iCalendar type symbol: %s" type))
   ((icalendar-value-type-symbol-p type)
    `(icalendar-ast-node-valid-p
      (icalendar-make-value-node-of (quote ,type) ,(car templates))))
   ((icalendar-param-type-symbol-p type)
    `(icalendar-make-param ,type ,(car templates)))
   ((icalendar-property-type-symbol-p type)
    `(icalendar-make-property ,type ,(car templates) ,@(cdr templates)))
   ((icalendar-component-type-symbol-p type)
    `(icalendar-make-component ,type ,@templates))))

(defmacro icalendar-make-vcalendar (&rest templates)
  "Construct an iCalendar VCALENDAR object from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vcalendar' for the permissible child types.

If TEMPLATES does not contain templates for the `icalendar-prodid' and
`icalendar-version' properties, they will be automatically added; see
the variables `icalendar-vcalendar-prodid' and
`icalendar-vcalendar-version'."
  `(icalendar-make-node-from-templates icalendar-vcalendar ,@templates))

(defmacro icalendar-make-vevent (&rest templates)
  "Construct an iCalendar VEVENT node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vevent' for the permissible child types.

If TEMPLATES does not contain templates for the `icalendar-dtstamp' and
`icalendar-uid' properties (both required), they will be automatically
provided."
  `(icalendar-make-node-from-templates icalendar-vevent ,@templates))

(defmacro icalendar-make-vtodo (&rest templates)
  "Construct an iCalendar VTODO node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vtodo' for the permissible child types.

If TEMPLATES does not contain templates for the `icalendar-dtstamp' and
`icalendar-uid' properties (both required), they will be automatically
provided."
  `(icalendar-make-node-from-templates icalendar-vtodo ,@templates))

(defmacro icalendar-make-vjournal (&rest templates)
  "Construct an iCalendar VJOURNAL node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vjournal' for the permissible child types.

If TEMPLATES does not contain templates for the `icalendar-dtstamp' and
`icalendar-uid' properties (both required), they will be automatically
provided."
  `(icalendar-make-node-from-templates icalendar-vjournal ,@templates))

(defmacro icalendar-make-vfreebusy (&rest templates)
  "Construct an iCalendar VFREEBUSY node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vfreebusy' for the permissible child types.

If TEMPLATES does not contain templates for the `icalendar-dtstamp' and
`icalendar-uid' properties (both required), they will be automatically
provided."
  `(icalendar-make-node-from-templates icalendar-vfreebusy ,@templates))

(defmacro icalendar-make-valarm (&rest templates)
  "Construct an iCalendar VALARM node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-valarm' for the permissible child types."
  `(icalendar-make-node-from-templates icalendar-valarm ,@templates))

(defmacro icalendar-make-vtimezone (&rest templates)
  "Construct an iCalendar VTIMEZONE node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-vtimezone' for the permissible child types."
  `(icalendar-make-node-from-templates icalendar-vtimezone ,@templates))

(defmacro icalendar-make-standard (&rest templates)
  "Construct an iCalendar STANDARD node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-standard' for the permissible child types."
  `(icalendar-make-node-from-templates icalendar-standard ,@templates))

(defmacro icalendar-make-daylight (&rest templates)
  "Construct an iCalendar DAYLIGHT node from TEMPLATES.
See `icalendar-make-node-from-templates' for the format of TEMPLATES.
See `icalendar-daylight' for the permissible child types."
  `(icalendar-make-node-from-templates icalendar-daylight ,@templates))


;;; Validation:

;; Errors at the validation stage:
;; e.g. property/param values did not match, or are of the wrong type,
;; or required properties not present in a component
(define-error 'icalendar-validation-error "Invalid iCalendar data" 'icalendar-error)

(cl-defun icalendar-signal-validation-error (msg &key node (severity 2))
  (signal 'icalendar-validation-error
              (list :message msg
                    :buffer (icalendar-ast-node-meta-get :buffer node)
                    :position (icalendar-ast-node-meta-get :begin node)
                    :severity severity
                    :node node)))

(defun icalendar-ast-node-required-child-p (child parent)
  "Return non-nil if CHILD is required by PARENT's node type."
  (let* ((type (icalendar-ast-node-type parent))
         (child-spec (get type 'icalendar-child-spec))
         (child-type (icalendar-ast-node-type child)))
    (or (memq child-type (plist-get child-spec :one))
        (memq child-type (plist-get child-spec :one-or-more)))))

(defun icalendar-ast-node-valid-value-p (node)
  "Validate that NODE's value satisfies the requirements of its type.
Signals an `icalendar-validation-error' if NODE's value is
invalid, or returns NODE."
  (require 'icalendar-parser) ; for icalendar-printable-value-type-symbol-p
  (declare-function icalendar-printable-value-type-symbol-p "icalendar-parser")
  (let* ((type (icalendar-ast-node-type node))
         (value (icalendar-ast-node-value node))
         (valtype-param (when (icalendar-property-type-symbol-p type)
                          (icalendar-with-param-of node 'icalendar-valuetypeparam)))
         (allowed-types
          (cond ((icalendar-printable-value-type-symbol-p valtype-param)
                 ;; with an explicit `VALUE=sometype' param, this is the
                 ;; only allowed type:
                 (list valtype-param))
                ((and (icalendar-param-type-symbol-p type)
                      (get type 'icalendar-value-type))
                 (list (get type 'icalendar-value-type)))
                ((icalendar-property-type-symbol-p type)
                 (cons (get type 'icalendar-default-type)
                       (get type 'icalendar-other-types)))
                (t nil))))
    (cond ((icalendar-value-type-symbol-p type)
           (unless (cl-typep value type) ; see `icalendar-define-type'
             (icalendar-signal-validation-error
              (format "Invalid value for `%s' node: %s" type value)
              :node node))
           node)
          ((icalendar-component-node-p node)
           ;; component types have no value, so no need to check anything
           node)
          ((and (or (icalendar-param-type-symbol-p type)
                    (icalendar-property-type-symbol-p type))
                (null (get type 'icalendar-value-type))
                (stringp value))
           ;; property and param nodes with no value type are assumed to contain
           ;; strings which match a value regex:
           (unless (string-match (rx-to-string (get type 'icalendar-value-rx)) value)
             (icalendar-signal-validation-error
              (format "Invalid string value for `%s' node: %s" type value)
              :node node))
           node)
          ;; otherwise this is a param or property node which itself
          ;; should have one or more syntax nodes as a value, so
          ;; recurse on value(s):
          ((icalendar-expects-list-of-values-p type)
           (unless (listp value)
             (icalendar-signal-validation-error
              (format "Expected list of values for `%s' node" type)
              :node node))
           (when allowed-types
             (dolist (v value)
               (unless (memq (icalendar-ast-node-type v) allowed-types)
                 (icalendar-signal-validation-error
                  (format "Value of unexpected type `%s' in `%s' node"
                          (icalendar-ast-node-type v) type)
                  :node node))))
           (mapc #'icalendar-ast-node-valid-value-p value)
           node)
          (t
           (unless (icalendar-ast-node-p value)
             (icalendar-signal-validation-error
              (format "Invalid value for `%s' node: %s" type value)
              :node node))
           (when allowed-types
             (unless (memq (icalendar-ast-node-type value) allowed-types)
               (icalendar-signal-validation-error
                (format "Value of unexpected type `%s' in `%s' node"
                        (icalendar-ast-node-type value) type)
                :node node)))
           (icalendar-ast-node-valid-value-p value)))))

(defun icalendar-count-children-by-type (node)
  "Count NODE's children by type.
Returns an alist mapping type symbols to the number of NODE's children
of that type."
  (let ((children (icalendar-ast-node-children node))
        (map nil))
    (dolist (child children map)
      (let* ((type (icalendar-ast-node-type child))
             (n (alist-get type map)))
        (setf (alist-get type map) (1+ (or n 0)))))))

(defun icalendar-ast-node-valid-children-p (node)
  "Validate that NODE's children satisfy its type's :child-spec.

The :child-spec is associated with NODE's type by
`icalendar-define-component', `icalendar-define-property',
`icalendar-define-param', or `icalendar-define-type', which see.
Signals an `icalendar-validation-error' if NODE is invalid, or returns
NODE.

Note that this function does not check that the children of NODE
are themselves valid; for that, see `icalendar-ast-node-valid-p'."
  (let* ((type (icalendar-ast-node-type node))
         (child-spec (get type 'icalendar-child-spec))
         (child-counts (icalendar-count-children-by-type node)))

    (when child-spec

      (dolist (child-type (plist-get child-spec :one))
        (unless (= 1 (alist-get child-type child-counts 0))
          (icalendar-signal-validation-error
            (format "iCalendar `%s' node must contain exactly one `%s'"
                    type child-type)
            :node node)))

      (dolist (child-type (plist-get child-spec :one-or-more))
        (unless (<= 1 (alist-get child-type child-counts 0))
          (icalendar-signal-validation-error
           (format "iCalendar `%s' node must contain one or more `%s'"
                   type child-type)
           :node node)))

      (dolist (child-type (plist-get child-spec :zero-or-one))
        (unless (<= (alist-get child-type child-counts 0)
                    1)
          (icalendar-signal-validation-error
           (format "iCalendar `%s' node may contain at most one `%s'"
                   type child-type)
           :node node)))

      ;; check that all child nodes are allowed:
      (unless (plist-get child-spec :allow-others)
        (let ((allowed-types (append (plist-get child-spec :one)
                                     (plist-get child-spec :one-or-more)
                                     (plist-get child-spec :zero-or-one)
                                     (plist-get child-spec :zero-or-more)))
              (appearing-types (mapcar #'car child-counts)))

          (dolist (child-type appearing-types)
            (unless (member child-type allowed-types)
              (icalendar-signal-validation-error
               (format "`%s' may not contain `%s'" type child-type)
               :node node))))))
    ;; success:
    node))

(defun icalendar-ast-node-valid-p (node &optional recursively)
  "Check that NODE is a valid iCalendar syntax node.
By default, the check will only validate NODE itself, but if
RECURSIVELY is non-nil, it will recursively check all its
descendants as well.  Signals an `icalendar-validation-error' if
NODE is invalid, or returns NODE."
  (unless (icalendar-ast-node-p node)
    (icalendar-signal-validation-error
     "Not an iCalendar syntax node"
     :node node))

  (icalendar-ast-node-valid-value-p node)
  (icalendar-ast-node-valid-children-p node)

  (let* ((type (icalendar-ast-node-type node))
         (other-validator (get type 'icalendar-other-validator)))

    (unless (icalendar-type-symbol-p type)
      (icalendar-signal-validation-error
       (format "Node's type `%s' is not an iCalendar type symbol" type)
       :node node))

    (when (and other-validator (not (functionp other-validator)))
      (icalendar-signal-validation-error
       (format "Bad validator function `%s' for type `%s'" other-validator type)))

    (when other-validator
      (funcall other-validator node)))

  (when recursively
    (dolist (c (icalendar-ast-node-children node))
      (icalendar-ast-node-valid-p c recursively)))

  ;; success:
  node)

(provide 'icalendar-ast)
;; Local Variables:
;; End:
;;; icalendar-ast.el ends here
