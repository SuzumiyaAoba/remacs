;;; -*- lexical-binding: t; -*-
;; Frame scroll-bar dimensions accept a frame object; non-frame values are
;; checked with GNU's `framep' predicate before any liveness check.
(qa (frame-scroll-bar-width nil) 0)
(qa (frame-scroll-bar-height nil) 0)
(qa (condition-case err
        (frame-scroll-bar-width "not-a-frame")
      (error (cdr err)))
    '(framep "not-a-frame"))
(qa (condition-case err
        (frame-scroll-bar-height "not-a-frame")
      (error (cdr err)))
    '(framep "not-a-frame"))

(princ (qa--done)) (terpri)
