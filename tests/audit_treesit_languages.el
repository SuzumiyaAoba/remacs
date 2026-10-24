;;; audit_treesit_languages.el --- installed grammar smoke tests -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(require 'treesit)
(setq treesit-extra-load-path (list (expand-file-name "~/.emacs.d/tree-sitter/")))
(defconst audit-grammar-languages '(awk bash bibtex blueprint c c-sharp clojure cmake commonlisp cpp css css-in-js dart dockerfile elixir glsl go gomod heex html janet java javascript json julia kotlin lua magik make nix nu org perl proto python r ruby rust scala sql surface toml tsx typescript typst verilog vhdl vue wast wat wgsl yaml))

(dolist (language audit-grammar-languages)
  (audit-check (format "treesit-grammar:%s" language)
    `(if (treesit-language-available-p ',language)
         (with-temp-buffer
           (insert "alpha = \"日本語\";\n")
           (let* ((parser (treesit-parser-create ',language))
                  (root (treesit-parser-root-node parser)))
             (list (treesit-node-type root)
                   (treesit-node-start root)
                   (treesit-node-end root)
                   (treesit-node-text root)
                   (and (treesit-query-capture root "(_) @node") t))))
       'unavailable)))

(princ "AUDIT-DONE\n")
