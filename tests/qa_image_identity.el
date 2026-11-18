;;; -*- lexical-binding: nil; -*-
;; Image tags, type names, and the :type property use canonical symbols.
(qa (image-type-available-p 'png) t)
(qa (image-type-available-p (make-symbol "png")) nil)
(qa (imagep '(image :type png :data "bytes")) t)
(qa (imagep (list (make-symbol "image") :type 'png :data "bytes")) nil)
(qa (imagep (list 'image (make-symbol ":type") 'png :data "bytes")) nil)
(qa (imagep (list 'image :type (make-symbol "png") :data "bytes")) nil)
;; GNU's image predicate recognizes source keys by their symbol names.
(qa (imagep (list 'image :type 'png (make-symbol ":data") "bytes")) t)
(princ (qa--done)) (terpri)
