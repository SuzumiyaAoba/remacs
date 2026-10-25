;;; -*- lexical-binding: nil; -*-
;; Random seeds use GNU string storage bytes, including unibyte 0x80/0xFF (#598).
(qa
 (let ((seed (unibyte-string 128 255)))
   (list (random seed) (random) (random 100) (random)))
 '(-695710751304136888 -1332926958776872202 19 724897448075017935))
(qa
 (mapcar (lambda (seed)
           (list (random seed) (random) (random 100) (random)))
         (list "abc" "é" (unibyte-string 128 255)
               (string-as-multibyte (unibyte-string 128 255))
               (string #x110000)))
 '((1568872268420745132 -75440345148207844 6 45380306580338996)
   (1721791314277868982 869041036555975279 41 397926003061494004)
   (-695710751304136888 -1332926958776872202 19 724897448075017935)
   (1668938789493238413 1516167988043508047 37 -1957761237951124824)
   (1602034738958514736 -2259057882330107581 23 -1803017768682853057)))
(princ (qa--done))
