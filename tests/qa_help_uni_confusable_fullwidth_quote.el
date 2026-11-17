;;; qa_help_uni_confusable_fullwidth_quote.el --- Unicode confusable regression -*- lexical-binding: t -*-

(require 'help)

(if qa--remacs
    (qa (help-uni-confusable-suggestions "＂")
        "Found confusable character: ‘＂’, perhaps you meant: ‘\"’?")
  ;; GNU 31.1 currently maps U+FF02 to an apostrophe.
  (qa-xf (help-uni-confusable-suggestions "＂")
         "Found confusable character: ‘＂’, perhaps you meant: ‘\"’?"
         "U+FF02 maps to U+0022 FULLWIDTH QUOTATION MARK"))

(princ (qa--done))
