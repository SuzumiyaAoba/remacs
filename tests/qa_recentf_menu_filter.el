;;; qa_recentf_menu_filter.el --- recentf menu filter regression -*- lexical-binding: t -*-

(require 'recentf)

(if qa--remacs
    (qa (let ((a (cons "a" "v1"))
              (submenu (cons "submenu" (list (cons "c" "v2"))))
              (b (cons "b" "v3")))
          (mapcar #'car
                  (recentf-apply-menu-filter #'identity
                                             (list a submenu b))))
        '("a" "submenu" "b"))
  ;; GNU 31.1 currently moves submenus before all ordinary elements.
  (qa-xf (let ((a (cons "a" "v1"))
           (submenu (cons "submenu" (list (cons "c" "v2"))))
           (b (cons "b" "v3")))
          (mapcar #'car
                  (recentf-apply-menu-filter #'identity
                                             (list a submenu b))))
        '("a" "submenu" "b")
        "Remacs preserves the original position of submenus"))

(princ (qa--done))
