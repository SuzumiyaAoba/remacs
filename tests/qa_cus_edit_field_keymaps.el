;;; qa_cus_edit_field_keymaps.el --- Customize field keymap isolation regression -*- lexical-binding:t -*-

(require 'wid-edit)

(qa
 (let* ((widget-type (get 'editable-field 'widget-type))
        (generic-keymap (widget-get widget-type :keymap)))
   (require 'cus-edit)
   (let* ((custom-keymap (widget-get widget-type :keymap))
          (custom-field
           (with-temp-buffer
             (setq major-mode 'Custom-mode)
             (let ((field (widget-create 'editable-field :value "custom")))
               (widget-setup)
               field)))
          (ordinary-field
           (with-temp-buffer
             (let ((field (widget-create 'editable-field :value "ordinary")))
               (widget-setup)
               field))))
     (list (eq generic-keymap custom-keymap)
           (not (eq (lookup-key generic-keymap "\C-c\C-c") 'Custom-set))
           (eq (widget-get custom-field :keymap) custom-field-keymap)
           (eq (lookup-key (widget-get custom-field :keymap) "\C-c\C-c")
               'Custom-set)
           (eq (widget-get ordinary-field :keymap) generic-keymap))))
 '(t t t t t))

(princ (qa--done))
