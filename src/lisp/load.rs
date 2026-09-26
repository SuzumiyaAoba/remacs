//! File loading: locate libraries on load-path, eval .el files.

use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Candidate paths GNU's `openp' would try for NAME in DIR.
/// NOSUFFIX opens NAME literally; MUST-SUFFIX requires NAME to carry
/// a recognized `.el'/`.elc' suffix already.
fn candidates(dir: &Path, name: &str, nosuffix: bool, mustsuffix: bool) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let base = dir.join(name);
    if nosuffix {
        v.push(base);
    } else if name.ends_with(".el") || name.ends_with(".elc") {
        v.push(base);
    } else if mustsuffix {
        // `openp' with a suffix predicate never opens the bare name.
    } else {
        v.push(base.with_extension("elc"));
        v.push(base.with_extension("el"));
        v.push(base);
    }
    v
}

/// Find library file NAME on load-path (or as an absolute/relative path).
pub(crate) fn locate(i: &mut Interp, name: &str) -> Option<String> {
    locate_opts(i, name, false, false)
}

/// `locate' honoring `load''s NOSUFFIX/MUST-SUFFIX arguments.
pub(crate) fn locate_opts(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    mustsuffix: bool,
) -> Option<String> {
    let p = Path::new(name);
    if p.is_absolute() || name.contains('/') || name.contains(std::path::MAIN_SEPARATOR) {
        for c in candidates(Path::new(""), name, nosuffix, mustsuffix) {
            if c.is_file() {
                return Some(c.to_string_lossy().into_owned());
            }
        }
        if !nosuffix && !mustsuffix && p.is_file() {
            return Some(name.to_string());
        }
        if p.is_absolute() {
            return None;
        }
        // Relative name with a directory part ("language/burmese",
        // "quail/arabic").  GNU resolves it against load-path entries
        // (`leim' + "quail/arabic" → leim/quail/arabic.el); try the
        // same against every search dir.
        for dir in search_dirs(i) {
            for c in candidates(&dir, name, nosuffix, mustsuffix) {
                if c.is_file() {
                    return Some(c.to_string_lossy().into_owned());
                }
            }
        }
        // Our bundled tree is flat under lisp/, with colliding
        // basenames renamed to SUBDIR-BASE.el; also try that spelling,
        // then the bare basename (unique basenames like "arabic").
        let flat = name.replace('/', "-");
        let base = name.rsplit('/').next().unwrap_or(name);
        for dir_s in builtin_dirs() {
            let dir = PathBuf::from(dir_s);
            for n in [&flat, base] {
                for c in candidates(&dir, n, nosuffix, mustsuffix) {
                    if c.is_file() {
                        return Some(c.to_string_lossy().into_owned());
                    }
                }
            }
        }
        return None;
    }
    // Search load-path (a list of strings).
    for dir in search_dirs(i) {
        for c in candidates(&dir, name, nosuffix, mustsuffix) {
            if c.is_file() {
                return Some(c.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// `load-path' dirs followed by the bundled lisp/ dirs.
fn search_dirs(i: &mut Interp) -> Vec<PathBuf> {
    let lp_id = i.intern("load-path");
    let lp = i.symbol_value(lp_id);
    let mut v: Vec<PathBuf> = lp
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| match d {
            Value::Str(s) => Some(PathBuf::from(s.borrow().as_str())),
            _ => None,
        })
        .collect();
    v.extend(builtin_dirs().into_iter().map(PathBuf::from));
    v
}

/// Directories searched for bundled libraries after `load-path'
/// (mirrors `locate''s built-in fallback): the exe-relative lisp/
/// dirs plus a cwd-relative "lisp" for development builds.
pub(crate) fn builtin_dirs() -> Vec<String> {
    let mut v = vec!["lisp".to_string()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exedir) = exe.parent() {
            v.push(exedir.join("../lisp").to_string_lossy().into_owned());
            v.push(
                exedir
                    .join("../share/remacs/lisp")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    v
}

/// Built-in Lisp libraries embedded in the binary (the repo's lisp/ dir),
/// so `load`/autoload works regardless of the process cwd.
static EMBEDDED_LISP: &[(&str, &str)] = &[
    ("avl-tree", include_str!("../../lisp/avl-tree.el")),
    ("cl-macs", include_str!("../../lisp/cl-macs.el")),
    ("cl-seq", include_str!("../../lisp/cl-seq.el")),
    ("cl-extra", include_str!("../../lisp/cl-extra.el")),
    ("color", include_str!("../../lisp/color.el")),
    ("dom", include_str!("../../lisp/dom.el")),
    ("easy-mmode", include_str!("../../lisp/easy-mmode.el")),
    ("eieio", include_str!("../../lisp/eieio.el")),
    ("face-remap", include_str!("../../lisp/face-remap.el")),
    ("gv", include_str!("../../lisp/gv.el")),
    ("hi-lock", include_str!("../../lisp/hi-lock.el")),
    ("icomplete", include_str!("../../lisp/icomplete.el")),
    ("imenu", include_str!("../../lisp/imenu.el")),
    ("kmacro", include_str!("../../lisp/kmacro.el")),
    ("let-alist", include_str!("../../lisp/let-alist.el")),
    ("macros", include_str!("../../lisp/macros.el")),
    ("map", include_str!("../../lisp/map.el")),
    ("minibuf-eldef", include_str!("../../lisp/minibuf-eldef.el")),
    ("pcase", include_str!("../../lisp/pcase.el")),
    ("pp", include_str!("../../lisp/pp.el")),
    ("pulse", include_str!("../../lisp/pulse.el")),
    ("repeat", include_str!("../../lisp/repeat.el")),
    ("ring", include_str!("../../lisp/ring.el")),
    ("subr-x", include_str!("../../lisp/subr-x.el")),
    ("thingatpt", include_str!("../../lisp/thingatpt.el")),
    ("thunk", include_str!("../../lisp/thunk.el")),
    ("time-date", include_str!("../../lisp/time-date.el")),
    ("savehist", include_str!("../../lisp/savehist.el")),
    ("saveplace", include_str!("../../lisp/saveplace.el")),
    ("mb-depth", include_str!("../../lisp/mb-depth.el")),
    ("radix-tree", include_str!("../../lisp/radix-tree.el")),
    ("skeleton", include_str!("../../lisp/skeleton.el")),
    ("doctor", include_str!("../../lisp/doctor.el")),
    ("mpuz", include_str!("../../lisp/mpuz.el")),
    ("rx", include_str!("../../lisp/rx.el")),
    (
        "tabulated-list",
        include_str!("../../lisp/tabulated-list.el"),
    ),
    ("tab-bar", include_str!("../../lisp/tab-bar.el")),
    (
        "display-line-numbers",
        include_str!("../../lisp/display-line-numbers.el"),
    ),
    ("buff-menu", include_str!("../../lisp/buff-menu.el")),
    ("icons", include_str!("../../lisp/icons.el")),
    ("image", include_str!("../../lisp/image.el")),
    ("flymake", include_str!("../../lisp/flymake.el")),
    ("warnings", include_str!("../../lisp/warnings.el")),
    ("ewoc", include_str!("../../lisp/ewoc.el")),
    ("ansi-color", include_str!("../../lisp/ansi-color.el")),
    ("regi", include_str!("../../lisp/regi.el")),
    ("tempo", include_str!("../../lisp/tempo.el")),
    ("autoinsert", include_str!("../../lisp/autoinsert.el")),
    ("dabbrev", include_str!("../../lisp/dabbrev.el")),
    ("expand", include_str!("../../lisp/expand.el")),
    ("tq", include_str!("../../lisp/tq.el")),
    ("tildify", include_str!("../../lisp/tildify.el")),
    ("timezone", include_str!("../../lisp/timezone.el")),
    ("cookie1", include_str!("../../lisp/cookie1.el")),
    ("env", include_str!("../../lisp/env.el")),
    ("novice", include_str!("../../lisp/novice.el")),
    ("delim-col", include_str!("../../lisp/delim-col.el")),
    ("align", include_str!("../../lisp/align.el")),
    ("filecache", include_str!("../../lisp/filecache.el")),
    ("rot13", include_str!("../../lisp/rot13.el")),
    ("soundex", include_str!("../../lisp/soundex.el")),
    ("hex-util", include_str!("../../lisp/hex-util.el")),
    (
        "password-cache",
        include_str!("../../lisp/password-cache.el"),
    ),
    ("indent-aux", include_str!("../../lisp/indent-aux.el")),
    ("underline", include_str!("../../lisp/underline.el")),
    ("studly", include_str!("../../lisp/studly.el")),
    ("dissociate", include_str!("../../lisp/dissociate.el")),
    ("talk", include_str!("../../lisp/talk.el")),
    ("spook", include_str!("../../lisp/spook.el")),
    ("ring-bell-fns", include_str!("../../lisp/ring-bell-fns.el")),
    ("gamegrid", include_str!("../../lisp/gamegrid.el")),
    ("gomoku", include_str!("../../lisp/gomoku.el")),
    ("hanoi", include_str!("../../lisp/hanoi.el")),
    ("life", include_str!("../../lisp/life.el")),
    ("solitaire", include_str!("../../lisp/solitaire.el")),
    ("md4", include_str!("../../lisp/md4.el")),
    ("elide-head", include_str!("../../lisp/elide-head.el")),
    (
        "external-completion",
        include_str!("../../lisp/external-completion.el"),
    ),
    ("case-table", include_str!("../../lisp/case-table.el")),
    ("chistory", include_str!("../../lisp/chistory.el")),
    ("midnight", include_str!("../../lisp/midnight.el")),
    ("cl-lib", include_str!("../../lisp/cl-lib.el")),
    ("cl-loaddefs", include_str!("../../lisp/cl-loaddefs.el")),
    ("loaddefs", include_str!("../../lisp/loaddefs.el")),
    ("cl-print", include_str!("../../lisp/cl-print.el")),
    ("filenotify", include_str!("../../lisp/filenotify.el")),
    ("autorevert", include_str!("../../lisp/autorevert.el")),
    ("dired", include_str!("../../lisp/dired.el")),
    ("frameset", include_str!("../../lisp/frameset.el")),
    ("desktop", include_str!("../../lisp/desktop.el")),
    ("diff-mode", include_str!("../../lisp/diff-mode.el")),
    ("track-changes", include_str!("../../lisp/track-changes.el")),
    ("format-spec", include_str!("../../lisp/format-spec.el")),
    ("tabify", include_str!("../../lisp/tabify.el")),
    ("scroll-lock", include_str!("../../lisp/scroll-lock.el")),
    ("obarray", include_str!("../../lisp/obarray.el")),
    ("ansi-osc", include_str!("../../lisp/ansi-osc.el")),
    ("flow-ctrl", include_str!("../../lisp/flow-ctrl.el")),
    ("help-macro", include_str!("../../lisp/help-macro.el")),
    ("master", include_str!("../../lisp/master.el")),
    ("rfn-eshadow", include_str!("../../lisp/rfn-eshadow.el")),
    ("scroll-all", include_str!("../../lisp/scroll-all.el")),
    ("send-to", include_str!("../../lisp/send-to.el")),
    ("userlock", include_str!("../../lisp/userlock.el")),
    ("delsel", include_str!("../../lisp/delsel.el")),
    ("help-at-pt", include_str!("../../lisp/help-at-pt.el")),
    ("apropos", include_str!("../../lisp/apropos.el")),
    ("rtree", include_str!("../../lisp/rtree.el")),
    ("time-stamp", include_str!("../../lisp/time-stamp.el")),
    ("emacs-lock", include_str!("../../lisp/emacs-lock.el")),
    ("mouse-copy", include_str!("../../lisp/mouse-copy.el")),
    ("t-mouse", include_str!("../../lisp/t-mouse.el")),
    ("bind-key", include_str!("../../lisp/bind-key.el")),
    ("misc", include_str!("../../lisp/misc.el")),
    ("mouse-drag", include_str!("../../lisp/mouse-drag.el")),
    ("lpr", include_str!("../../lisp/lpr.el")),
    ("loadhist", include_str!("../../lisp/loadhist.el")),
    ("jka-cmpr-hook", include_str!("../../lisp/jka-cmpr-hook.el")),
    ("jka-compr", include_str!("../../lisp/jka-compr.el")),
    ("hl-line", include_str!("../../lisp/hl-line.el")),
    ("ecomplete", include_str!("../../lisp/ecomplete.el")),
    ("avoid", include_str!("../../lisp/avoid.el")),
    ("visual-wrap", include_str!("../../lisp/visual-wrap.el")),
    ("disp-table", include_str!("../../lisp/disp-table.el")),
    ("window-x", include_str!("../../lisp/window-x.el")),
    ("xt-mouse", include_str!("../../lisp/xt-mouse.el")),
    ("tmm", include_str!("../../lisp/tmm.el")),
    (
        "text-property-search",
        include_str!("../../lisp/text-property-search.el"),
    ),
    ("yank-media", include_str!("../../lisp/yank-media.el")),
    ("bs", include_str!("../../lisp/bs.el")),
    (
        "editorconfig-fnmatch",
        include_str!("../../lisp/editorconfig-fnmatch.el"),
    ),
    (
        "editorconfig-core-handle",
        include_str!("../../lisp/editorconfig-core-handle.el"),
    ),
    (
        "editorconfig-core",
        include_str!("../../lisp/editorconfig-core.el"),
    ),
    ("editorconfig", include_str!("../../lisp/editorconfig.el")),
    (
        "editorconfig-tools",
        include_str!("../../lisp/editorconfig-tools.el"),
    ),
    (
        "editorconfig-conf-mode",
        include_str!("../../lisp/editorconfig-conf-mode.el"),
    ),
    ("conf-mode", include_str!("../../lisp/conf-mode.el")),
    ("project", include_str!("../../lisp/project.el")),
    ("vc-hooks", include_str!("../../lisp/vc-hooks.el")),
    ("vc-dispatcher", include_str!("../../lisp/vc-dispatcher.el")),
    ("vc", include_str!("../../lisp/vc.el")),
    ("vc-dir", include_str!("../../lisp/vc-dir.el")),
    ("vc-git", include_str!("../../lisp/vc-git.el")),
    ("proced", include_str!("../../lisp/proced.el")),
    ("server", include_str!("../../lisp/server.el")),
    ("wid-edit", include_str!("../../lisp/wid-edit.el")),
    ("tree-widget", include_str!("../../lisp/tree-widget.el")),
    ("recentf", include_str!("../../lisp/recentf.el")),
    ("comint", include_str!("../../lisp/comint.el")),
    ("compile", include_str!("../../lisp/compile.el")),
    ("epg-config", include_str!("../../lisp/epg-config.el")),
    ("array", include_str!("../../lisp/array.el")),
    ("dos-vars", include_str!("../../lisp/dos-vars.el")),
    ("fringe", include_str!("../../lisp/fringe.el")),
    ("font-core", include_str!("../../lisp/font-core.el")),
    (
        "dynamic-setting",
        include_str!("../../lisp/dynamic-setting.el"),
    ),
    ("double", include_str!("../../lisp/double.el")),
    ("generator", include_str!("../../lisp/generator.el")),
    ("fileloop", include_str!("../../lisp/fileloop.el")),
    (
        "display-fill-column-indicator",
        include_str!("../../lisp/display-fill-column-indicator.el"),
    ),
    ("widget", include_str!("../../lisp/widget.el")),
    ("w32-vars", include_str!("../../lisp/w32-vars.el")),
    ("version", include_str!("../../lisp/version.el")),
    ("tool-bar", include_str!("../../lisp/tool-bar.el")),
    ("reposition", include_str!("../../lisp/reposition.el")),
    ("reveal", include_str!("../../lisp/reveal.el")),
    ("dos-fns", include_str!("../../lisp/dos-fns.el")),
    ("sqlite", include_str!("../../lisp/sqlite.el")),
    ("sqlite-mode", include_str!("../../lisp/sqlite-mode.el")),
    ("xdg", include_str!("../../lisp/xdg.el")),
    ("tty-tip", include_str!("../../lisp/tty-tip.el")),
    ("isearch-x", include_str!("../../lisp/isearch-x.el")),
    ("ibuf-macs", include_str!("../../lisp/ibuf-macs.el")),
    ("w32-fns", include_str!("../../lisp/w32-fns.el")),
    ("ebuff-menu", include_str!("../../lisp/ebuff-menu.el")),
    ("zone", include_str!("../../lisp/zone.el")),
    ("tooltip", include_str!("../../lisp/tooltip.el")),
    ("ogonek", include_str!("../../lisp/ogonek.el")),
    ("kinsoku", include_str!("../../lisp/kinsoku.el")),
    ("utf-7", include_str!("../../lisp/utf-7.el")),
    ("ls-lisp", include_str!("../../lisp/ls-lisp.el")),
    ("mail-utils", include_str!("../../lisp/mail-utils.el")),
    ("fortune", include_str!("../../lisp/fortune.el")),
    ("morse", include_str!("../../lisp/morse.el")),
    ("eieio-base", include_str!("../../lisp/eieio-base.el")),
    ("registry", include_str!("../../lisp/registry.el")),
    ("float-sup", include_str!("../../lisp/float-sup.el")),
    ("map-ynp", include_str!("../../lisp/map-ynp.el")),
    ("benchmark", include_str!("../../lisp/benchmark.el")),
    ("helper", include_str!("../../lisp/helper.el")),
    ("page", include_str!("../../lisp/page.el")),
    ("animate", include_str!("../../lisp/animate.el")),
    ("cursor-sensor", include_str!("../../lisp/cursor-sensor.el")),
    ("timer-list", include_str!("../../lisp/timer-list.el")),
    ("bib-mode", include_str!("../../lisp/bib-mode.el")),
    ("string-edit", include_str!("../../lisp/string-edit.el")),
    ("shadow", include_str!("../../lisp/shadow.el")),
    ("inline", include_str!("../../lisp/inline.el")),
    ("compat", include_str!("../../lisp/compat.el")),
    ("refill", include_str!("../../lisp/refill.el")),
    (
        "word-wrap-mode",
        include_str!("../../lisp/word-wrap-mode.el"),
    ),
    (
        "glyphless-mode",
        include_str!("../../lisp/glyphless-mode.el"),
    ),
    ("crm", include_str!("../../lisp/crm.el")),
    ("timeout", include_str!("../../lisp/timeout.el")),
    ("pixel-fill", include_str!("../../lisp/pixel-fill.el")),
    ("po", include_str!("../../lisp/po.el")),
    ("bibtex-style", include_str!("../../lisp/bibtex-style.el")),
    (
        "emacs-authors-mode",
        include_str!("../../lisp/emacs-authors-mode.el"),
    ),
    ("ld-script", include_str!("../../lisp/ld-script.el")),
    ("m4-mode", include_str!("../../lisp/m4-mode.el")),
    ("bat-mode", include_str!("../../lisp/bat-mode.el")),
    ("asm-mode", include_str!("../../lisp/asm-mode.el")),
    ("cl-font-lock", include_str!("../../lisp/cl-font-lock.el")),
    ("autoconf", include_str!("../../lisp/autoconf.el")),
    ("trace", include_str!("../../lisp/trace.el")),
    ("memory-report", include_str!("../../lisp/memory-report.el")),
    ("executable", include_str!("../../lisp/executable.el")),
    (
        "generate-lisp-file",
        include_str!("../../lisp/generate-lisp-file.el"),
    ),
    ("debug-early", include_str!("../../lisp/debug-early.el")),
    ("rfc1843", include_str!("../../lisp/rfc1843.el")),
    ("ja-dic-utl", include_str!("../../lisp/ja-dic-utl.el")),
    ("iso-ascii", include_str!("../../lisp/iso-ascii.el")),
    ("latexenc", include_str!("../../lisp/latexenc.el")),
    ("rfc6068", include_str!("../../lisp/rfc6068.el")),
    ("yenc", include_str!("../../lisp/yenc.el")),
    ("qp", include_str!("../../lisp/qp.el")),
    ("flow-fill", include_str!("../../lisp/flow-fill.el")),
    ("mailheader", include_str!("../../lisp/mailheader.el")),
    ("uudecode", include_str!("../../lisp/uudecode.el")),
    ("binhex", include_str!("../../lisp/binhex.el")),
    ("mail-prsvr", include_str!("../../lisp/mail-prsvr.el")),
    ("ietf-drums", include_str!("../../lisp/ietf-drums.el")),
    ("rfc2045", include_str!("../../lisp/rfc2045.el")),
    ("mm-util", include_str!("../../lisp/mm-util.el")),
    ("rfc2047", include_str!("../../lisp/rfc2047.el")),
    ("rfc2231", include_str!("../../lisp/rfc2231.el")),
    ("mail-parse", include_str!("../../lisp/mail-parse.el")),
    ("cond-star", include_str!("../../lisp/cond-star.el")),
    ("derived", include_str!("../../lisp/derived.el")),
    ("range", include_str!("../../lisp/range.el")),
    ("generic", include_str!("../../lisp/generic.el")),
    ("copyright", include_str!("../../lisp/copyright.el")),
    ("msb", include_str!("../../lisp/msb.el")),
    ("rect", include_str!("../../lisp/rect.el")),
    ("xml", include_str!("../../lisp/xml.el")),
    ("uniquify", include_str!("../../lisp/uniquify.el")),
    ("view", include_str!("../../lisp/view.el")),
    ("whitespace", include_str!("../../lisp/whitespace.el")),
    ("windmove", include_str!("../../lisp/windmove.el")),
    ("winner", include_str!("../../lisp/winner.el")),
    ("paren", include_str!("../../lisp/paren.el")),
    ("elec-pair", include_str!("../../lisp/elec-pair.el")),
    ("electric", include_str!("../../lisp/electric.el")),
    ("follow", include_str!("../../lisp/follow.el")),
    ("completion", include_str!("../../lisp/completion.el")),
    (
        "completion-preview",
        include_str!("../../lisp/completion-preview.el"),
    ),
    ("glasses", include_str!("../../lisp/glasses.el")),
    ("dcl-mode", include_str!("../../lisp/dcl-mode.el")),
    ("cpp", include_str!("../../lisp/cpp.el")),
    ("bug-reference", include_str!("../../lisp/bug-reference.el")),
    ("faceup", include_str!("../../lisp/faceup.el")),
    ("hierarchy", include_str!("../../lisp/hierarchy.el")),
    ("multisession", include_str!("../../lisp/multisession.el")),
    ("rmc", include_str!("../../lisp/rmc.el")),
    ("re-builder", include_str!("../../lisp/re-builder.el")),
    ("check-declare", include_str!("../../lisp/check-declare.el")),
    ("cl-indent", include_str!("../../lisp/cl-indent.el")),
    ("find-func", include_str!("../../lisp/find-func.el")),
    ("lisp-mnt", include_str!("../../lisp/lisp-mnt.el")),
    ("vtable", include_str!("../../lisp/vtable.el")),
    ("autoarg", include_str!("../../lisp/autoarg.el")),
    ("blackbox", include_str!("../../lisp/blackbox.el")),
    ("cdl", include_str!("../../lisp/cdl.el")),
    ("decipher", include_str!("../../lisp/decipher.el")),
    ("dig", include_str!("../../lisp/dig.el")),
    ("dns-mode", include_str!("../../lisp/dns-mode.el")),
    ("dns", include_str!("../../lisp/dns.el")),
    ("dunnet", include_str!("../../lisp/dunnet.el")),
    ("epa-hook", include_str!("../../lisp/epa-hook.el")),
    ("eudc-vars", include_str!("../../lisp/eudc-vars.el")),
    ("format", include_str!("../../lisp/format.el")),
    ("forms", include_str!("../../lisp/forms.el")),
    ("gs", include_str!("../../lisp/gs.el")),
    ("gulp", include_str!("../../lisp/gulp.el")),
    ("hfy-cmap", include_str!("../../lisp/hfy-cmap.el")),
    ("hmac-def", include_str!("../../lisp/hmac-def.el")),
    ("html2text", include_str!("../../lisp/html2text.el")),
    ("icon", include_str!("../../lisp/icon.el")),
    ("iso-cvt", include_str!("../../lisp/iso-cvt.el")),
    ("iso-transl", include_str!("../../lisp/iso-transl.el")),
    ("linum", include_str!("../../lisp/linum.el")),
    ("longlines", include_str!("../../lisp/longlines.el")),
    ("mail-extr", include_str!("../../lisp/mail-extr.el")),
    ("mailcap", include_str!("../../lisp/mailcap.el")),
    ("makesum", include_str!("../../lisp/makesum.el")),
    ("mantemp", include_str!("../../lisp/mantemp.el")),
    ("meta-mode", include_str!("../../lisp/meta-mode.el")),
    ("metamail", include_str!("../../lisp/metamail.el")),
    ("mixal-mode", include_str!("../../lisp/mixal-mode.el")),
    ("mule-util", include_str!("../../lisp/mule-util.el")),
    ("net-utils", include_str!("../../lisp/net-utils.el")),
    ("netrc", include_str!("../../lisp/netrc.el")),
    ("opascal", include_str!("../../lisp/opascal.el")),
    ("page-ext", include_str!("../../lisp/page-ext.el")),
    ("pascal", include_str!("../../lisp/pascal.el")),
    ("pgg-def", include_str!("../../lisp/pgg-def.el")),
    ("pgg-parse", include_str!("../../lisp/pgg-parse.el")),
    ("picture", include_str!("../../lisp/picture.el")),
    ("ps-def", include_str!("../../lisp/ps-def.el")),
    ("refbib", include_str!("../../lisp/refbib.el")),
    ("rfc2368", include_str!("../../lisp/rfc2368.el")),
    ("rfc822", include_str!("../../lisp/rfc822.el")),
    ("robin", include_str!("../../lisp/robin.el")),
    ("sasl", include_str!("../../lisp/sasl.el")),
    ("shortdoc-doc", include_str!("../../lisp/shortdoc-doc.el")),
    ("sieve-mode", include_str!("../../lisp/sieve-mode.el")),
    ("simula", include_str!("../../lisp/simula.el")),
    ("starttls", include_str!("../../lisp/starttls.el")),
    ("subword", include_str!("../../lisp/subword.el")),
    ("tramp-uu", include_str!("../../lisp/tramp-uu.el")),
    ("trampver", include_str!("../../lisp/trampver.el")),
    ("type-break", include_str!("../../lisp/type-break.el")),
    ("url-dired", include_str!("../../lisp/url-dired.el")),
    ("vt-control", include_str!("../../lisp/vt-control.el")),
    ("vt100-led", include_str!("../../lisp/vt100-led.el")),
    ("composite", include_str!("../../lisp/composite.el")),
    ("shortdoc", include_str!("../../lisp/shortdoc.el")),
    ("add-log", include_str!("../../lisp/add-log.el")),
    ("arc-mode", include_str!("../../lisp/arc-mode.el")),
    ("bruce", include_str!("../../lisp/bruce.el")),
    ("bubbles", include_str!("../../lisp/bubbles.el")),
    ("cfengine", include_str!("../../lisp/cfengine.el")),
    ("compare-w", include_str!("../../lisp/compare-w.el")),
    ("diff", include_str!("../../lisp/diff.el")),
    ("dos-w32", include_str!("../../lisp/dos-w32.el")),
    ("echistory", include_str!("../../lisp/echistory.el")),
    ("ehelp", include_str!("../../lisp/ehelp.el")),
    ("elint", include_str!("../../lisp/elint.el")),
    ("emerge", include_str!("../../lisp/emerge.el")),
    ("epg", include_str!("../../lisp/epg.el")),
    ("facemenu", include_str!("../../lisp/facemenu.el")),
    ("filesets", include_str!("../../lisp/filesets.el")),
    ("find-dired", include_str!("../../lisp/find-dired.el")),
    ("find-lisp", include_str!("../../lisp/find-lisp.el")),
    ("flymake-cc", include_str!("../../lisp/flymake-cc.el")),
    ("footnote", include_str!("../../lisp/footnote.el")),
    ("goto-addr", include_str!("../../lisp/goto-addr.el")),
    ("hashcash", include_str!("../../lisp/hashcash.el")),
    ("help-mode", include_str!("../../lisp/help-mode.el")),
    ("hideshow", include_str!("../../lisp/hideshow.el")),
    ("hmac-md5", include_str!("../../lisp/hmac-md5.el")),
    ("ido", include_str!("../../lisp/ido.el")),
    ("iswitchb", include_str!("../../lisp/iswitchb.el")),
    ("landmark", include_str!("../../lisp/landmark.el")),
    ("ldap", include_str!("../../lisp/ldap.el")),
    ("loaddefs-gen", include_str!("../../lisp/loaddefs-gen.el")),
    ("make-mode", include_str!("../../lisp/make-mode.el")),
    ("man", include_str!("../../lisp/man.el")),
    ("misearch", include_str!("../../lisp/misearch.el")),
    ("nroff-mode", include_str!("../../lisp/nroff-mode.el")),
    ("nsm", include_str!("../../lisp/nsm.el")),
    ("perl-mode", include_str!("../../lisp/perl-mode.el")),
    ("pixel-scroll", include_str!("../../lisp/pixel-scroll.el")),
    ("pong", include_str!("../../lisp/pong.el")),
    ("puny", include_str!("../../lisp/puny.el")),
    ("reftex-vars", include_str!("../../lisp/reftex-vars.el")),
    ("reporter", include_str!("../../lisp/reporter.el")),
    ("rfc2104", include_str!("../../lisp/rfc2104.el")),
    ("scheme", include_str!("../../lisp/scheme.el")),
    ("sendmail", include_str!("../../lisp/sendmail.el")),
    ("snake", include_str!("../../lisp/snake.el")),
    ("snmp-mode", include_str!("../../lisp/snmp-mode.el")),
    ("socks", include_str!("../../lisp/socks.el")),
    ("strokes", include_str!("../../lisp/strokes.el")),
    ("supercite", include_str!("../../lisp/supercite.el")),
    ("svg", include_str!("../../lisp/svg.el")),
    ("tetris", include_str!("../../lisp/tetris.el")),
    ("thumbs", include_str!("../../lisp/thumbs.el")),
    ("touch-screen", include_str!("../../lisp/touch-screen.el")),
    ("tutorial", include_str!("../../lisp/tutorial.el")),
    ("two-column", include_str!("../../lisp/two-column.el")),
    ("utf7", include_str!("../../lisp/utf7.el")),
    ("vc-bzr", include_str!("../../lisp/vc-bzr.el")),
    ("vc-hg", include_str!("../../lisp/vc-hg.el")),
    ("vc-svn", include_str!("../../lisp/vc-svn.el")),
    ("wdired", include_str!("../../lisp/wdired.el")),
    ("which-key", include_str!("../../lisp/which-key.el")),
    ("x-dnd", include_str!("../../lisp/x-dnd.el")),
    ("abbrev", include_str!("../../lisp/abbrev.el")),
    ("cconv", include_str!("../../lisp/cconv.el")),
    ("cus-face", include_str!("../../lisp/cus-face.el")),
    ("cus-edit", include_str!("../../lisp/cus-edit.el")),
    ("ediff-hook", include_str!("../../lisp/ediff-hook.el")),
    ("eldoc", include_str!("../../lisp/eldoc.el")),
    ("mouse", include_str!("../../lisp/mouse.el")),
    ("prog-mode", include_str!("../../lisp/prog-mode.el")),
    ("regexp-opt", include_str!("../../lisp/regexp-opt.el")),
    ("register", include_str!("../../lisp/register.el")),
    ("replace", include_str!("../../lisp/replace.el")),
    ("scroll-bar", include_str!("../../lisp/scroll-bar.el")),
    ("text-mode", include_str!("../../lisp/text-mode.el")),
    ("timer", include_str!("../../lisp/timer.el")),
    ("plstore", include_str!("../../lisp/plstore.el")),
    ("profiler", include_str!("../../lisp/profiler.el")),
    ("vcursor", include_str!("../../lisp/vcursor.el")),
    ("epa", include_str!("../../lisp/epa.el")),
    ("artist", include_str!("../../lisp/artist.el")),
    ("enriched", include_str!("../../lisp/enriched.el")),
    ("paragraphs", include_str!("../../lisp/paragraphs.el")),
    ("sgml-mode", include_str!("../../lisp/sgml-mode.el")),
    ("fill", include_str!("../../lisp/fill.el")),
    ("xscheme", include_str!("../../lisp/xscheme.el")),
    ("ebrowse", include_str!("../../lisp/ebrowse.el")),
    ("f90", include_str!("../../lisp/f90.el")),
    ("fortran", include_str!("../../lisp/fortran.el")),
    ("which-func", include_str!("../../lisp/which-func.el")),
    ("etags-regen", include_str!("../../lisp/etags-regen.el")),
    ("vc-filewise", include_str!("../../lisp/vc-filewise.el")),
    ("vc-src", include_str!("../../lisp/vc-src.el")),
    ("pcvs-util", include_str!("../../lisp/pcvs-util.el")),
    ("vc-annotate", include_str!("../../lisp/vc-annotate.el")),
    ("cua-base", include_str!("../../lisp/cua-base.el")),
    ("keypad", include_str!("../../lisp/keypad.el")),
    ("edt", include_str!("../../lisp/edt.el")),
    ("viper-init", include_str!("../../lisp/viper-init.el")),
    ("tab-line", include_str!("../../lisp/tab-line.el")),
    ("time", include_str!("../../lisp/time.el")),
    ("woman", include_str!("../../lisp/woman.el")),
    ("bookmark", include_str!("../../lisp/bookmark.el")),
    ("char-fold", include_str!("../../lisp/char-fold.el")),
    ("select", include_str!("../../lisp/select.el")),
    ("newcomment", include_str!("../../lisp/newcomment.el")),
    ("json", include_str!("../../lisp/json.el")),
    ("sort", include_str!("../../lisp/sort.el")),
    ("dired-x", include_str!("../../lisp/dired-x.el")),
    ("find-file", include_str!("../../lisp/find-file.el")),
    ("help-fns", include_str!("../../lisp/help-fns.el")),
    ("chart", include_str!("../../lisp/chart.el")),
    ("tar-mode", include_str!("../../lisp/tar-mode.el")),
    ("elisp-scope", include_str!("../../lisp/elisp-scope.el")),
    (
        "allout-widgets",
        include_str!("../../lisp/allout-widgets.el"),
    ),
    ("allout", include_str!("../../lisp/allout.el")),
    ("auth-source", include_str!("../../lisp/auth-source.el")),
    ("calculator", include_str!("../../lisp/calculator.el")),
    ("cmuscheme", include_str!("../../lisp/cmuscheme.el")),
    ("cperl-mode", include_str!("../../lisp/cperl-mode.el")),
    ("cus-dep", include_str!("../../lisp/cus-dep.el")),
    ("cus-start", include_str!("../../lisp/cus-start.el")),
    ("cus-theme", include_str!("../../lisp/cus-theme.el")),
    ("descr-text", include_str!("../../lisp/descr-text.el")),
    ("dired-aux", include_str!("../../lisp/dired-aux.el")),
    ("dirtrack", include_str!("../../lisp/dirtrack.el")),
    ("ezimage", include_str!("../../lisp/ezimage.el")),
    ("files-x", include_str!("../../lisp/files-x.el")),
    ("find-cmd", include_str!("../../lisp/find-cmd.el")),
    ("flymake-proc", include_str!("../../lisp/flymake-proc.el")),
    ("flyspell", include_str!("../../lisp/flyspell.el")),
    ("foldout", include_str!("../../lisp/foldout.el")),
    ("grep", include_str!("../../lisp/grep.el")),
    ("hippie-exp", include_str!("../../lisp/hippie-exp.el")),
    ("ielm", include_str!("../../lisp/ielm.el")),
    (
        "ietf-drums-date",
        include_str!("../../lisp/ietf-drums-date.el"),
    ),
    ("iimage", include_str!("../../lisp/iimage.el")),
    ("image-file", include_str!("../../lisp/image-file.el")),
    ("isearchb", include_str!("../../lisp/isearchb.el")),
    ("iso8601", include_str!("../../lisp/iso8601.el")),
    ("ispell", include_str!("../../lisp/ispell.el")),
    ("locate", include_str!("../../lisp/locate.el")),
    ("modula2", include_str!("../../lisp/modula2.el")),
    ("octave", include_str!("../../lisp/octave.el")),
    ("outline", include_str!("../../lisp/outline.el")),
    ("parse-time", include_str!("../../lisp/parse-time.el")),
    ("pcomplete", include_str!("../../lisp/pcomplete.el")),
    ("prolog", include_str!("../../lisp/prolog.el")),
    ("ps-samp", include_str!("../../lisp/ps-samp.el")),
    ("quail", include_str!("../../lisp/quail.el")),
    (
        "reftex-loaddefs",
        include_str!("../../lisp/reftex-loaddefs.el"),
    ),
    ("reftex", include_str!("../../lisp/reftex.el")),
    ("remember", include_str!("../../lisp/remember.el")),
    ("ruby-mode", include_str!("../../lisp/ruby-mode.el")),
    ("ruler-mode", include_str!("../../lisp/ruler-mode.el")),
    ("shell", include_str!("../../lisp/shell.el")),
    ("smie", include_str!("../../lisp/smie.el")),
    ("so-long", include_str!("../../lisp/so-long.el")),
    ("sql", include_str!("../../lisp/sql.el")),
    ("table", include_str!("../../lisp/table.el")),
    ("tcl", include_str!("../../lisp/tcl.el")),
    ("tex-mode", include_str!("../../lisp/tex-mode.el")),
    (
        "texinfo-loaddefs",
        include_str!("../../lisp/texinfo-loaddefs.el"),
    ),
    ("texinfo", include_str!("../../lisp/texinfo.el")),
    ("thread", include_str!("../../lisp/thread.el")),
    ("verilog-mode", include_str!("../../lisp/verilog-mode.el")),
    ("wid-browse", include_str!("../../lisp/wid-browse.el")),
    ("blessmail", include_str!("../../lisp/blessmail.el")),
    ("shorthands", include_str!("../../lisp/shorthands.el")),
    (
        "image-converter",
        include_str!("../../lisp/image-converter.el"),
    ),
    ("ps-print", include_str!("../../lisp/ps-print.el")),
    (
        "ps-print-loaddefs",
        include_str!("../../lisp/ps-print-loaddefs.el"),
    ),
    ("printing", include_str!("../../lisp/printing.el")),
    ("zeroconf", include_str!("../../lisp/zeroconf.el")),
    ("dbus", include_str!("../../lisp/dbus.el")),
    ("backtrace", include_str!("../../lisp/backtrace.el")),
    ("appt", include_str!("../../lisp/appt.el")),
    ("cal-bahai", include_str!("../../lisp/cal-bahai.el")),
    ("cal-china", include_str!("../../lisp/cal-china.el")),
    ("cal-coptic", include_str!("../../lisp/cal-coptic.el")),
    ("cal-dst", include_str!("../../lisp/cal-dst.el")),
    ("cal-french", include_str!("../../lisp/cal-french.el")),
    ("cal-hebrew", include_str!("../../lisp/cal-hebrew.el")),
    ("cal-html", include_str!("../../lisp/cal-html.el")),
    ("cal-islam", include_str!("../../lisp/cal-islam.el")),
    ("cal-iso", include_str!("../../lisp/cal-iso.el")),
    ("cal-julian", include_str!("../../lisp/cal-julian.el")),
    ("cal-loaddefs", include_str!("../../lisp/cal-loaddefs.el")),
    ("cal-mayan", include_str!("../../lisp/cal-mayan.el")),
    ("cal-menu", include_str!("../../lisp/cal-menu.el")),
    ("cal-move", include_str!("../../lisp/cal-move.el")),
    ("cal-persia", include_str!("../../lisp/cal-persia.el")),
    ("cal-tex", include_str!("../../lisp/cal-tex.el")),
    ("cal-x", include_str!("../../lisp/cal-x.el")),
    ("calendar", include_str!("../../lisp/calendar.el")),
    ("diary-lib", include_str!("../../lisp/diary-lib.el")),
    (
        "diary-loaddefs",
        include_str!("../../lisp/diary-loaddefs.el"),
    ),
    (
        "holiday-loaddefs",
        include_str!("../../lisp/holiday-loaddefs.el"),
    ),
    ("holidays", include_str!("../../lisp/holidays.el")),
    ("lunar", include_str!("../../lisp/lunar.el")),
    ("solar", include_str!("../../lisp/solar.el")),
    ("timeclock", include_str!("../../lisp/timeclock.el")),
    ("todo-mode", include_str!("../../lisp/todo-mode.el")),
    (
        "use-package-bind-key",
        include_str!("../../lisp/use-package-bind-key.el"),
    ),
    (
        "use-package-core",
        include_str!("../../lisp/use-package-core.el"),
    ),
    (
        "use-package-delight",
        include_str!("../../lisp/use-package-delight.el"),
    ),
    (
        "use-package-diminish",
        include_str!("../../lisp/use-package-diminish.el"),
    ),
    (
        "use-package-ensure-system-package",
        include_str!("../../lisp/use-package-ensure-system-package.el"),
    ),
    (
        "use-package-ensure",
        include_str!("../../lisp/use-package-ensure.el"),
    ),
    (
        "use-package-jump",
        include_str!("../../lisp/use-package-jump.el"),
    ),
    (
        "use-package-lint",
        include_str!("../../lisp/use-package-lint.el"),
    ),
    ("use-package", include_str!("../../lisp/use-package.el")),
    ("AT386", include_str!("../../lisp/AT386.el")),
    ("advice", include_str!("../../lisp/advice.el")),
    ("android-win", include_str!("../../lisp/android-win.el")),
    ("antlr-mode", include_str!("../../lisp/antlr-mode.el")),
    ("autoload", include_str!("../../lisp/autoload.el")),
    ("backquote", include_str!("../../lisp/backquote.el")),
    ("bindat", include_str!("../../lisp/bindat.el")),
    ("bobcat", include_str!("../../lisp/bobcat.el")),
    ("browse-url", include_str!("../../lisp/browse-url.el")),
    ("byte-opt", include_str!("../../lisp/byte-opt.el")),
    ("byte-run", include_str!("../../lisp/byte-run.el")),
    ("bytecomp", include_str!("../../lisp/bytecomp.el")),
    ("c-ts-common", include_str!("../../lisp/c-ts-common.el")),
    ("c-ts-mode", include_str!("../../lisp/c-ts-mode.el")),
    ("calc", include_str!("../../lisp/calc.el")),
    ("calc-aent", include_str!("../../lisp/calc-aent.el")),
    ("calc-alg", include_str!("../../lisp/calc-alg.el")),
    ("calc-arith", include_str!("../../lisp/calc-arith.el")),
    ("calc-bin", include_str!("../../lisp/calc-bin.el")),
    ("calc-comb", include_str!("../../lisp/calc-comb.el")),
    ("calc-cplx", include_str!("../../lisp/calc-cplx.el")),
    ("calc-embed", include_str!("../../lisp/calc-embed.el")),
    ("calc-ext", include_str!("../../lisp/calc-ext.el")),
    ("calc-fin", include_str!("../../lisp/calc-fin.el")),
    ("calc-forms", include_str!("../../lisp/calc-forms.el")),
    ("calc-frac", include_str!("../../lisp/calc-frac.el")),
    ("calc-funcs", include_str!("../../lisp/calc-funcs.el")),
    ("calc-graph", include_str!("../../lisp/calc-graph.el")),
    ("calc-help", include_str!("../../lisp/calc-help.el")),
    ("calc-incom", include_str!("../../lisp/calc-incom.el")),
    ("calc-keypd", include_str!("../../lisp/calc-keypd.el")),
    ("calc-lang", include_str!("../../lisp/calc-lang.el")),
    ("calc-loaddefs", include_str!("../../lisp/calc-loaddefs.el")),
    ("calc-macs", include_str!("../../lisp/calc-macs.el")),
    ("calc-map", include_str!("../../lisp/calc-map.el")),
    ("calc-math", include_str!("../../lisp/calc-math.el")),
    ("calc-menu", include_str!("../../lisp/calc-menu.el")),
    ("calc-misc", include_str!("../../lisp/calc-misc.el")),
    ("calc-mode", include_str!("../../lisp/calc-mode.el")),
    ("calc-mtx", include_str!("../../lisp/calc-mtx.el")),
    ("calc-nlfit", include_str!("../../lisp/calc-nlfit.el")),
    ("calc-poly", include_str!("../../lisp/calc-poly.el")),
    ("calc-prog", include_str!("../../lisp/calc-prog.el")),
    ("calc-rewr", include_str!("../../lisp/calc-rewr.el")),
    ("calc-rules", include_str!("../../lisp/calc-rules.el")),
    ("calc-sel", include_str!("../../lisp/calc-sel.el")),
    ("calc-stat", include_str!("../../lisp/calc-stat.el")),
    ("calc-store", include_str!("../../lisp/calc-store.el")),
    ("calc-stuff", include_str!("../../lisp/calc-stuff.el")),
    ("calc-trail", include_str!("../../lisp/calc-trail.el")),
    ("calc-undo", include_str!("../../lisp/calc-undo.el")),
    ("calc-units", include_str!("../../lisp/calc-units.el")),
    ("calc-vec", include_str!("../../lisp/calc-vec.el")),
    ("calc-yank", include_str!("../../lisp/calc-yank.el")),
    ("calcalg2", include_str!("../../lisp/calcalg2.el")),
    ("calcalg3", include_str!("../../lisp/calcalg3.el")),
    ("calccomp", include_str!("../../lisp/calccomp.el")),
    ("calcsel2", include_str!("../../lisp/calcsel2.el")),
    ("cc-align", include_str!("../../lisp/cc-align.el")),
    ("cc-awk", include_str!("../../lisp/cc-awk.el")),
    ("cc-bytecomp", include_str!("../../lisp/cc-bytecomp.el")),
    ("cc-cmds", include_str!("../../lisp/cc-cmds.el")),
    ("cc-defs", include_str!("../../lisp/cc-defs.el")),
    ("cc-engine", include_str!("../../lisp/cc-engine.el")),
    ("cc-fonts", include_str!("../../lisp/cc-fonts.el")),
    ("cc-guess", include_str!("../../lisp/cc-guess.el")),
    ("cc-langs", include_str!("../../lisp/cc-langs.el")),
    ("cc-menus", include_str!("../../lisp/cc-menus.el")),
    ("cc-mode", include_str!("../../lisp/cc-mode.el")),
    ("cc-styles", include_str!("../../lisp/cc-styles.el")),
    ("cc-vars", include_str!("../../lisp/cc-vars.el")),
    ("ccl", include_str!("../../lisp/ccl.el")),
    ("characters", include_str!("../../lisp/characters.el")),
    ("charprop", include_str!("../../lisp/charprop.el")),
    ("charscript", include_str!("../../lisp/charscript.el")),
    ("checkdoc", include_str!("../../lisp/checkdoc.el")),
    ("cl", include_str!("../../lisp/cl.el")),
    ("cl-generic", include_str!("../../lisp/cl-generic.el")),
    ("cl-preloaded", include_str!("../../lisp/cl-preloaded.el")),
    ("cmacexp", include_str!("../../lisp/cmacexp.el")),
    ("cmake-ts-mode", include_str!("../../lisp/cmake-ts-mode.el")),
    ("common-win", include_str!("../../lisp/common-win.el")),
    ("comp", include_str!("../../lisp/comp.el")),
    ("comp-common", include_str!("../../lisp/comp-common.el")),
    ("comp-cstr", include_str!("../../lisp/comp-cstr.el")),
    ("comp-run", include_str!("../../lisp/comp-run.el")),
    ("compface", include_str!("../../lisp/compface.el")),
    ("cp51932", include_str!("../../lisp/cp51932.el")),
    ("csharp-mode", include_str!("../../lisp/csharp-mode.el")),
    ("css-mode", include_str!("../../lisp/css-mode.el")),
    ("cua-gmrk", include_str!("../../lisp/cua-gmrk.el")),
    ("cua-rect", include_str!("../../lisp/cua-rect.el")),
    ("cvs-status", include_str!("../../lisp/cvs-status.el")),
    ("cwarn", include_str!("../../lisp/cwarn.el")),
    ("cygwin", include_str!("../../lisp/cygwin.el")),
    ("debug", include_str!("../../lisp/debug.el")),
    ("disass", include_str!("../../lisp/disass.el")),
    (
        "dockerfile-ts-mode",
        include_str!("../../lisp/dockerfile-ts-mode.el"),
    ),
    ("easymenu", include_str!("../../lisp/easymenu.el")),
    ("edebug", include_str!("../../lisp/edebug.el")),
    ("edt-lk201", include_str!("../../lisp/edt-lk201.el")),
    ("edt-mapper", include_str!("../../lisp/edt-mapper.el")),
    ("edt-pc", include_str!("../../lisp/edt-pc.el")),
    ("edt-vt100", include_str!("../../lisp/edt-vt100.el")),
    ("eglot", include_str!("../../lisp/eglot.el")),
    ("eieio-compat", include_str!("../../lisp/eieio-compat.el")),
    ("eieio-core", include_str!("../../lisp/eieio-core.el")),
    ("eieio-custom", include_str!("../../lisp/eieio-custom.el")),
    (
        "eieio-datadebug",
        include_str!("../../lisp/eieio-datadebug.el"),
    ),
    ("eieio-opt", include_str!("../../lisp/eieio-opt.el")),
    (
        "eieio-speedbar",
        include_str!("../../lisp/eieio-speedbar.el"),
    ),
    ("elisp-mode", include_str!("../../lisp/elisp-mode.el")),
    (
        "elixir-ts-mode",
        include_str!("../../lisp/elixir-ts-mode.el"),
    ),
    ("emacsbug", include_str!("../../lisp/emacsbug.el")),
    ("emoji", include_str!("../../lisp/emoji.el")),
    ("emoji-labels", include_str!("../../lisp/emoji-labels.el")),
    ("emoji-zwj", include_str!("../../lisp/emoji-zwj.el")),
    ("erc", include_str!("../../lisp/erc.el")),
    ("erc-autoaway", include_str!("../../lisp/erc-autoaway.el")),
    ("erc-backend", include_str!("../../lisp/erc-backend.el")),
    ("erc-button", include_str!("../../lisp/erc-button.el")),
    ("erc-capab", include_str!("../../lisp/erc-capab.el")),
    ("erc-common", include_str!("../../lisp/erc-common.el")),
    ("erc-compat", include_str!("../../lisp/erc-compat.el")),
    ("erc-dcc", include_str!("../../lisp/erc-dcc.el")),
    (
        "erc-desktop-notifications",
        include_str!("../../lisp/erc-desktop-notifications.el"),
    ),
    ("erc-ezbounce", include_str!("../../lisp/erc-ezbounce.el")),
    ("erc-fill", include_str!("../../lisp/erc-fill.el")),
    ("erc-goodies", include_str!("../../lisp/erc-goodies.el")),
    ("erc-ibuffer", include_str!("../../lisp/erc-ibuffer.el")),
    ("erc-identd", include_str!("../../lisp/erc-identd.el")),
    ("erc-imenu", include_str!("../../lisp/erc-imenu.el")),
    ("erc-join", include_str!("../../lisp/erc-join.el")),
    ("erc-lang", include_str!("../../lisp/erc-lang.el")),
    ("erc-list", include_str!("../../lisp/erc-list.el")),
    ("erc-loaddefs", include_str!("../../lisp/erc-loaddefs.el")),
    ("erc-log", include_str!("../../lisp/erc-log.el")),
    ("erc-match", include_str!("../../lisp/erc-match.el")),
    ("erc-menu", include_str!("../../lisp/erc-menu.el")),
    ("erc-netsplit", include_str!("../../lisp/erc-netsplit.el")),
    ("erc-networks", include_str!("../../lisp/erc-networks.el")),
    ("erc-nicks", include_str!("../../lisp/erc-nicks.el")),
    ("erc-notify", include_str!("../../lisp/erc-notify.el")),
    ("erc-page", include_str!("../../lisp/erc-page.el")),
    ("erc-pcomplete", include_str!("../../lisp/erc-pcomplete.el")),
    ("erc-replace", include_str!("../../lisp/erc-replace.el")),
    ("erc-ring", include_str!("../../lisp/erc-ring.el")),
    ("erc-sasl", include_str!("../../lisp/erc-sasl.el")),
    ("erc-services", include_str!("../../lisp/erc-services.el")),
    ("erc-sound", include_str!("../../lisp/erc-sound.el")),
    ("erc-speedbar", include_str!("../../lisp/erc-speedbar.el")),
    ("erc-spelling", include_str!("../../lisp/erc-spelling.el")),
    ("erc-stamp", include_str!("../../lisp/erc-stamp.el")),
    (
        "erc-status-sidebar",
        include_str!("../../lisp/erc-status-sidebar.el"),
    ),
    ("erc-track", include_str!("../../lisp/erc-track.el")),
    ("erc-truncate", include_str!("../../lisp/erc-truncate.el")),
    ("erc-xdcc", include_str!("../../lisp/erc-xdcc.el")),
    ("ert", include_str!("../../lisp/ert.el")),
    ("ert-font-lock", include_str!("../../lisp/ert-font-lock.el")),
    ("ert-x", include_str!("../../lisp/ert-x.el")),
    ("erts-mode", include_str!("../../lisp/erts-mode.el")),
    ("etags", include_str!("../../lisp/etags.el")),
    ("eucjp-ms", include_str!("../../lisp/eucjp-ms.el")),
    ("eudc", include_str!("../../lisp/eudc.el")),
    ("eudc-bob", include_str!("../../lisp/eudc-bob.el")),
    ("eudc-capf", include_str!("../../lisp/eudc-capf.el")),
    ("eudc-export", include_str!("../../lisp/eudc-export.el")),
    ("eudc-hotlist", include_str!("../../lisp/eudc-hotlist.el")),
    ("eudcb-bbdb", include_str!("../../lisp/eudcb-bbdb.el")),
    (
        "eudcb-ecomplete",
        include_str!("../../lisp/eudcb-ecomplete.el"),
    ),
    ("eudcb-ldap", include_str!("../../lisp/eudcb-ldap.el")),
    ("eudcb-mab", include_str!("../../lisp/eudcb-mab.el")),
    (
        "eudcb-macos-contacts",
        include_str!("../../lisp/eudcb-macos-contacts.el"),
    ),
    (
        "eudcb-mailabbrev",
        include_str!("../../lisp/eudcb-mailabbrev.el"),
    ),
    ("eudcb-ph", include_str!("../../lisp/eudcb-ph.el")),
    ("eww", include_str!("../../lisp/eww.el")),
    ("fbterm", include_str!("../../lisp/fbterm.el")),
    ("feedmail", include_str!("../../lisp/feedmail.el")),
    ("fontset", include_str!("../../lisp/fontset.el")),
    ("gdb-mi", include_str!("../../lisp/gdb-mi.el")),
    ("gnus-dbus", include_str!("../../lisp/gnus-dbus.el")),
    ("gnutls", include_str!("../../lisp/gnutls.el")),
    ("go-ts-mode", include_str!("../../lisp/go-ts-mode.el")),
    ("gravatar", include_str!("../../lisp/gravatar.el")),
    ("gud", include_str!("../../lisp/gud.el")),
    ("haiku-win", include_str!("../../lisp/haiku-win.el")),
    ("heex-ts-mode", include_str!("../../lisp/heex-ts-mode.el")),
    ("hideif", include_str!("../../lisp/hideif.el")),
    ("html-ts-mode", include_str!("../../lisp/html-ts-mode.el")),
    (
        "idlw-complete-structtag",
        include_str!("../../lisp/idlw-complete-structtag.el"),
    ),
    ("idlw-help", include_str!("../../lisp/idlw-help.el")),
    ("idlw-shell", include_str!("../../lisp/idlw-shell.el")),
    ("idlw-toolbar", include_str!("../../lisp/idlw-toolbar.el")),
    ("idlwave", include_str!("../../lisp/idlwave.el")),
    ("idna-mapping", include_str!("../../lisp/idna-mapping.el")),
    ("image-crop", include_str!("../../lisp/image-crop.el")),
    ("image-dired", include_str!("../../lisp/image-dired.el")),
    (
        "image-dired-dired",
        include_str!("../../lisp/image-dired-dired.el"),
    ),
    (
        "image-dired-external",
        include_str!("../../lisp/image-dired-external.el"),
    ),
    (
        "image-dired-tags",
        include_str!("../../lisp/image-dired-tags.el"),
    ),
    (
        "image-dired-util",
        include_str!("../../lisp/image-dired-util.el"),
    ),
    ("imap", include_str!("../../lisp/imap.el")),
    ("internal", include_str!("../../lisp/internal.el")),
    ("inversion", include_str!("../../lisp/inversion.el")),
    ("iris-ansi", include_str!("../../lisp/iris-ansi.el")),
    ("ja-dic-cnv", include_str!("../../lisp/ja-dic-cnv.el")),
    ("java-ts-mode", include_str!("../../lisp/java-ts-mode.el")),
    ("js", include_str!("../../lisp/js.el")),
    ("json-ts-mode", include_str!("../../lisp/json-ts-mode.el")),
    ("kkc", include_str!("../../lisp/kkc.el")),
    ("konsole", include_str!("../../lisp/konsole.el")),
    ("latin1-disp", include_str!("../../lisp/latin1-disp.el")),
    ("less-css-mode", include_str!("../../lisp/less-css-mode.el")),
    ("linux", include_str!("../../lisp/linux.el")),
    ("lisp", include_str!("../../lisp/lisp.el")),
    ("lisp-mode", include_str!("../../lisp/lisp-mode.el")),
    ("lk201", include_str!("../../lisp/lk201.el")),
    ("lua-ts-mode", include_str!("../../lisp/lua-ts-mode.el")),
    ("macroexp", include_str!("../../lisp/macroexp.el")),
    ("mailclient", include_str!("../../lisp/mailclient.el")),
    ("mairix", include_str!("../../lisp/mairix.el")),
    ("makeinfo", include_str!("../../lisp/makeinfo.el")),
    (
        "markdown-ts-mode",
        include_str!("../../lisp/markdown-ts-mode.el"),
    ),
    (
        "markdown-ts-mode-x",
        include_str!("../../lisp/markdown-ts-mode-x.el"),
    ),
    ("messcompat", include_str!("../../lisp/messcompat.el")),
    ("mh-acros", include_str!("../../lisp/mh-acros.el")),
    ("mh-alias", include_str!("../../lisp/mh-alias.el")),
    ("mh-buffers", include_str!("../../lisp/mh-buffers.el")),
    ("mh-comp", include_str!("../../lisp/mh-comp.el")),
    ("mh-compat", include_str!("../../lisp/mh-compat.el")),
    ("mh-e", include_str!("../../lisp/mh-e.el")),
    ("mh-folder", include_str!("../../lisp/mh-folder.el")),
    ("mh-funcs", include_str!("../../lisp/mh-funcs.el")),
    ("mh-gnus", include_str!("../../lisp/mh-gnus.el")),
    ("mh-identity", include_str!("../../lisp/mh-identity.el")),
    ("mh-inc", include_str!("../../lisp/mh-inc.el")),
    ("mh-junk", include_str!("../../lisp/mh-junk.el")),
    ("mh-letter", include_str!("../../lisp/mh-letter.el")),
    ("mh-limit", include_str!("../../lisp/mh-limit.el")),
    ("mh-loaddefs", include_str!("../../lisp/mh-loaddefs.el")),
    ("mh-mime", include_str!("../../lisp/mh-mime.el")),
    ("mh-print", include_str!("../../lisp/mh-print.el")),
    ("mh-scan", include_str!("../../lisp/mh-scan.el")),
    ("mh-search", include_str!("../../lisp/mh-search.el")),
    ("mh-seq", include_str!("../../lisp/mh-seq.el")),
    ("mh-show", include_str!("../../lisp/mh-show.el")),
    ("mh-speed", include_str!("../../lisp/mh-speed.el")),
    ("mh-thread", include_str!("../../lisp/mh-thread.el")),
    ("mh-tool-bar", include_str!("../../lisp/mh-tool-bar.el")),
    ("mh-utils", include_str!("../../lisp/mh-utils.el")),
    ("mh-xface", include_str!("../../lisp/mh-xface.el")),
    ("mhtml-mode", include_str!("../../lisp/mhtml-mode.el")),
    ("mhtml-ts-mode", include_str!("../../lisp/mhtml-ts-mode.el")),
    ("mspools", include_str!("../../lisp/mspools.el")),
    ("mule", include_str!("../../lisp/mule.el")),
    ("mule-cmds", include_str!("../../lisp/mule-cmds.el")),
    ("mule-conf", include_str!("../../lisp/mule-conf.el")),
    ("mule-diag", include_str!("../../lisp/mule-diag.el")),
    ("nadvice", include_str!("../../lisp/nadvice.el")),
    (
        "network-stream",
        include_str!("../../lisp/network-stream.el"),
    ),
    ("news", include_str!("../../lisp/news.el")),
    ("nnir", include_str!("../../lisp/nnir.el")),
    ("ns-win", include_str!("../../lisp/ns-win.el")),
    ("ntlm", include_str!("../../lisp/ntlm.el")),
    ("nxml-enc", include_str!("../../lisp/nxml-enc.el")),
    ("nxml-maint", include_str!("../../lisp/nxml-maint.el")),
    ("nxml-mode", include_str!("../../lisp/nxml-mode.el")),
    ("nxml-ns", include_str!("../../lisp/nxml-ns.el")),
    ("nxml-outln", include_str!("../../lisp/nxml-outln.el")),
    ("nxml-parse", include_str!("../../lisp/nxml-parse.el")),
    ("nxml-rap", include_str!("../../lisp/nxml-rap.el")),
    ("nxml-util", include_str!("../../lisp/nxml-util.el")),
    ("oclosure", include_str!("../../lisp/oclosure.el")),
    ("package", include_str!("../../lisp/package.el")),
    (
        "package-activate",
        include_str!("../../lisp/package-activate.el"),
    ),
    ("package-vc", include_str!("../../lisp/package-vc.el")),
    ("package-x", include_str!("../../lisp/package-x.el")),
    ("pc-win", include_str!("../../lisp/pc-win.el")),
    ("pcvs", include_str!("../../lisp/pcvs.el")),
    ("pcvs-defs", include_str!("../../lisp/pcvs-defs.el")),
    ("pcvs-info", include_str!("../../lisp/pcvs-info.el")),
    ("pcvs-parse", include_str!("../../lisp/pcvs-parse.el")),
    ("pgg", include_str!("../../lisp/pgg.el")),
    ("pgg-gpg", include_str!("../../lisp/pgg-gpg.el")),
    ("pgg-pgp", include_str!("../../lisp/pgg-pgp.el")),
    ("pgg-pgp5", include_str!("../../lisp/pgg-pgp5.el")),
    ("pgtk-win", include_str!("../../lisp/pgtk-win.el")),
    ("php-ts-mode", include_str!("../../lisp/php-ts-mode.el")),
    ("pop3", include_str!("../../lisp/pop3.el")),
    ("python", include_str!("../../lisp/python.el")),
    ("quickurl", include_str!("../../lisp/quickurl.el")),
    ("rcirc", include_str!("../../lisp/rcirc.el")),
    ("reftex-auc", include_str!("../../lisp/reftex-auc.el")),
    ("reftex-cite", include_str!("../../lisp/reftex-cite.el")),
    ("reftex-dcr", include_str!("../../lisp/reftex-dcr.el")),
    ("reftex-global", include_str!("../../lisp/reftex-global.el")),
    ("reftex-index", include_str!("../../lisp/reftex-index.el")),
    ("reftex-parse", include_str!("../../lisp/reftex-parse.el")),
    ("reftex-ref", include_str!("../../lisp/reftex-ref.el")),
    ("reftex-sel", include_str!("../../lisp/reftex-sel.el")),
    ("reftex-toc", include_str!("../../lisp/reftex-toc.el")),
    ("rlogin", include_str!("../../lisp/rlogin.el")),
    ("rmail", include_str!("../../lisp/rmail.el")),
    (
        "rmail-spam-filter",
        include_str!("../../lisp/rmail-spam-filter.el"),
    ),
    ("rmailedit", include_str!("../../lisp/rmailedit.el")),
    ("rmailkwd", include_str!("../../lisp/rmailkwd.el")),
    ("rmailmm", include_str!("../../lisp/rmailmm.el")),
    ("rmailmsc", include_str!("../../lisp/rmailmsc.el")),
    ("rmailout", include_str!("../../lisp/rmailout.el")),
    ("rmailsort", include_str!("../../lisp/rmailsort.el")),
    ("rmailsum", include_str!("../../lisp/rmailsum.el")),
    ("rng-cmpct", include_str!("../../lisp/rng-cmpct.el")),
    ("rng-dt", include_str!("../../lisp/rng-dt.el")),
    ("rng-loc", include_str!("../../lisp/rng-loc.el")),
    ("rng-maint", include_str!("../../lisp/rng-maint.el")),
    ("rng-match", include_str!("../../lisp/rng-match.el")),
    ("rng-nxml", include_str!("../../lisp/rng-nxml.el")),
    ("rng-parse", include_str!("../../lisp/rng-parse.el")),
    ("rng-pttrn", include_str!("../../lisp/rng-pttrn.el")),
    ("rng-uri", include_str!("../../lisp/rng-uri.el")),
    ("rng-util", include_str!("../../lisp/rng-util.el")),
    ("rng-valid", include_str!("../../lisp/rng-valid.el")),
    ("rng-xsd", include_str!("../../lisp/rng-xsd.el")),
    ("rst", include_str!("../../lisp/rst.el")),
    ("ruby-ts-mode", include_str!("../../lisp/ruby-ts-mode.el")),
    ("rust-ts-mode", include_str!("../../lisp/rust-ts-mode.el")),
    ("rxvt", include_str!("../../lisp/rxvt.el")),
    ("sasl-cram", include_str!("../../lisp/sasl-cram.el")),
    ("sasl-digest", include_str!("../../lisp/sasl-digest.el")),
    ("sasl-ntlm", include_str!("../../lisp/sasl-ntlm.el")),
    (
        "sasl-scram-rfc",
        include_str!("../../lisp/sasl-scram-rfc.el"),
    ),
    (
        "sasl-scram-sha256",
        include_str!("../../lisp/sasl-scram-sha256.el"),
    ),
    ("sb-image", include_str!("../../lisp/sb-image.el")),
    ("screen", include_str!("../../lisp/screen.el")),
    ("secrets", include_str!("../../lisp/secrets.el")),
    ("sh-script", include_str!("../../lisp/sh-script.el")),
    ("shr", include_str!("../../lisp/shr.el")),
    ("shr-color", include_str!("../../lisp/shr-color.el")),
    ("sieve", include_str!("../../lisp/sieve.el")),
    ("sieve-manage", include_str!("../../lisp/sieve-manage.el")),
    ("smerge-mode", include_str!("../../lisp/smerge-mode.el")),
    ("soap-client", include_str!("../../lisp/soap-client.el")),
    ("soap-inspect", include_str!("../../lisp/soap-inspect.el")),
    ("st", include_str!("../../lisp/st.el")),
    ("sun", include_str!("../../lisp/sun.el")),
    ("syntax", include_str!("../../lisp/syntax.el")),
    ("tcover-ses", include_str!("../../lisp/tcover-ses.el")),
    ("telnet", include_str!("../../lisp/telnet.el")),
    ("testcover", include_str!("../../lisp/testcover.el")),
    ("textsec", include_str!("../../lisp/textsec.el")),
    ("textsec-check", include_str!("../../lisp/textsec-check.el")),
    ("tls", include_str!("../../lisp/tls.el")),
    ("tmux", include_str!("../../lisp/tmux.el")),
    ("toml-ts-mode", include_str!("../../lisp/toml-ts-mode.el")),
    ("tpu-edt", include_str!("../../lisp/tpu-edt.el")),
    ("tpu-extras", include_str!("../../lisp/tpu-extras.el")),
    ("tpu-mapper", include_str!("../../lisp/tpu-mapper.el")),
    ("tramp", include_str!("../../lisp/tramp.el")),
    ("tramp-adb", include_str!("../../lisp/tramp-adb.el")),
    (
        "tramp-androidsu",
        include_str!("../../lisp/tramp-androidsu.el"),
    ),
    ("tramp-archive", include_str!("../../lisp/tramp-archive.el")),
    ("tramp-cache", include_str!("../../lisp/tramp-cache.el")),
    ("tramp-cmds", include_str!("../../lisp/tramp-cmds.el")),
    ("tramp-compat", include_str!("../../lisp/tramp-compat.el")),
    (
        "tramp-container",
        include_str!("../../lisp/tramp-container.el"),
    ),
    ("tramp-crypt", include_str!("../../lisp/tramp-crypt.el")),
    ("tramp-ftp", include_str!("../../lisp/tramp-ftp.el")),
    ("tramp-fuse", include_str!("../../lisp/tramp-fuse.el")),
    ("tramp-gvfs", include_str!("../../lisp/tramp-gvfs.el")),
    (
        "tramp-integration",
        include_str!("../../lisp/tramp-integration.el"),
    ),
    (
        "tramp-loaddefs",
        include_str!("../../lisp/tramp-loaddefs.el"),
    ),
    ("tramp-message", include_str!("../../lisp/tramp-message.el")),
    ("tramp-rclone", include_str!("../../lisp/tramp-rclone.el")),
    ("tramp-sh", include_str!("../../lisp/tramp-sh.el")),
    ("tramp-smb", include_str!("../../lisp/tramp-smb.el")),
    ("tramp-sshfs", include_str!("../../lisp/tramp-sshfs.el")),
    (
        "tramp-sudoedit",
        include_str!("../../lisp/tramp-sudoedit.el"),
    ),
    ("tty-colors", include_str!("../../lisp/tty-colors.el")),
    ("tvi970", include_str!("../../lisp/tvi970.el")),
    (
        "typescript-ts-mode",
        include_str!("../../lisp/typescript-ts-mode.el"),
    ),
    ("uce", include_str!("../../lisp/uce.el")),
    ("ucs-normalize", include_str!("../../lisp/ucs-normalize.el")),
    ("undigest", include_str!("../../lisp/undigest.el")),
    ("uni-bidi", include_str!("../../lisp/uni-bidi.el")),
    ("uni-brackets", include_str!("../../lisp/uni-brackets.el")),
    ("uni-category", include_str!("../../lisp/uni-category.el")),
    ("uni-combining", include_str!("../../lisp/uni-combining.el")),
    ("uni-comment", include_str!("../../lisp/uni-comment.el")),
    (
        "uni-confusable",
        include_str!("../../lisp/uni-confusable.el"),
    ),
    ("uni-decimal", include_str!("../../lisp/uni-decimal.el")),
    (
        "uni-decomposition",
        include_str!("../../lisp/uni-decomposition.el"),
    ),
    ("uni-digit", include_str!("../../lisp/uni-digit.el")),
    ("uni-lowercase", include_str!("../../lisp/uni-lowercase.el")),
    ("uni-mirrored", include_str!("../../lisp/uni-mirrored.el")),
    ("uni-name", include_str!("../../lisp/uni-name.el")),
    ("uni-numeric", include_str!("../../lisp/uni-numeric.el")),
    ("uni-old-name", include_str!("../../lisp/uni-old-name.el")),
    ("uni-scripts", include_str!("../../lisp/uni-scripts.el")),
    (
        "uni-special-lowercase",
        include_str!("../../lisp/uni-special-lowercase.el"),
    ),
    (
        "uni-special-titlecase",
        include_str!("../../lisp/uni-special-titlecase.el"),
    ),
    (
        "uni-special-uppercase",
        include_str!("../../lisp/uni-special-uppercase.el"),
    ),
    ("uni-titlecase", include_str!("../../lisp/uni-titlecase.el")),
    ("uni-uppercase", include_str!("../../lisp/uni-uppercase.el")),
    ("unrmail", include_str!("../../lisp/unrmail.el")),
    ("unsafep", include_str!("../../lisp/unsafep.el")),
    ("url-about", include_str!("../../lisp/url-about.el")),
    ("url-ns", include_str!("../../lisp/url-ns.el")),
    ("vc-arch", include_str!("../../lisp/vc-arch.el")),
    ("vc-cvs", include_str!("../../lisp/vc-cvs.el")),
    ("vc-dav", include_str!("../../lisp/vc-dav.el")),
    ("vc-mtn", include_str!("../../lisp/vc-mtn.el")),
    ("vc-rcs", include_str!("../../lisp/vc-rcs.el")),
    ("vc-sccs", include_str!("../../lisp/vc-sccs.el")),
    ("vera-mode", include_str!("../../lisp/vera-mode.el")),
    ("vhdl-mode", include_str!("../../lisp/vhdl-mode.el")),
    ("viper", include_str!("../../lisp/viper.el")),
    ("viper-cmd", include_str!("../../lisp/viper-cmd.el")),
    ("viper-ex", include_str!("../../lisp/viper-ex.el")),
    ("viper-keym", include_str!("../../lisp/viper-keym.el")),
    ("viper-macs", include_str!("../../lisp/viper-macs.el")),
    ("viper-mous", include_str!("../../lisp/viper-mous.el")),
    ("viper-util", include_str!("../../lisp/viper-util.el")),
    ("vt100", include_str!("../../lisp/vt100.el")),
    ("vt200", include_str!("../../lisp/vt200.el")),
    ("w32-nt", include_str!("../../lisp/w32-nt.el")),
    ("w32-win", include_str!("../../lisp/w32-win.el")),
    ("w32console", include_str!("../../lisp/w32console.el")),
    ("wallpaper", include_str!("../../lisp/wallpaper.el")),
    ("webjump", include_str!("../../lisp/webjump.el")),
    ("wyse50", include_str!("../../lisp/wyse50.el")),
    ("x-win", include_str!("../../lisp/x-win.el")),
    ("xmltok", include_str!("../../lisp/xmltok.el")),
    ("xref", include_str!("../../lisp/xref.el")),
    ("xsd-regexp", include_str!("../../lisp/xsd-regexp.el")),
    ("xterm", include_str!("../../lisp/xterm.el")),
    ("yaml-ts-mode", include_str!("../../lisp/yaml-ts-mode.el")),
    ("burmese", include_str!("../../lisp/burmese.el")),
    ("cham", include_str!("../../lisp/cham.el")),
    ("china-util", include_str!("../../lisp/china-util.el")),
    ("chinese", include_str!("../../lisp/chinese.el")),
    ("cyril-util", include_str!("../../lisp/cyril-util.el")),
    ("cyrillic", include_str!("../../lisp/cyrillic.el")),
    ("czech", include_str!("../../lisp/czech.el")),
    ("english", include_str!("../../lisp/english.el")),
    ("european", include_str!("../../lisp/european.el")),
    ("georgian", include_str!("../../lisp/georgian.el")),
    ("greek", include_str!("../../lisp/greek.el")),
    ("hanja-util", include_str!("../../lisp/hanja-util.el")),
    ("hebrew", include_str!("../../lisp/hebrew.el")),
    ("indian", include_str!("../../lisp/indian.el")),
    ("indonesian", include_str!("../../lisp/indonesian.el")),
    ("japan-util", include_str!("../../lisp/japan-util.el")),
    ("japanese", include_str!("../../lisp/japanese.el")),
    ("khmer", include_str!("../../lisp/khmer.el")),
    ("korea-util", include_str!("../../lisp/korea-util.el")),
    ("korean", include_str!("../../lisp/korean.el")),
    ("lao", include_str!("../../lisp/lao.el")),
    ("lao-util", include_str!("../../lisp/lao-util.el")),
    ("misc-lang", include_str!("../../lisp/misc-lang.el")),
    ("philippine", include_str!("../../lisp/philippine.el")),
    ("pinyin", include_str!("../../lisp/pinyin.el")),
    ("romanian", include_str!("../../lisp/romanian.el")),
    ("sinhala", include_str!("../../lisp/sinhala.el")),
    ("slovak", include_str!("../../lisp/slovak.el")),
    ("tai-viet", include_str!("../../lisp/tai-viet.el")),
    ("thai", include_str!("../../lisp/thai.el")),
    ("thai-util", include_str!("../../lisp/thai-util.el")),
    ("thai-word", include_str!("../../lisp/thai-word.el")),
    ("tv-util", include_str!("../../lisp/tv-util.el")),
    ("utf-8-lang", include_str!("../../lisp/utf-8-lang.el")),
    ("viet-util", include_str!("../../lisp/viet-util.el")),
    ("vietnamese", include_str!("../../lisp/vietnamese.el")),
    ("4Corner", include_str!("../../lisp/4Corner.el")),
    ("CCDOSPY", include_str!("../../lisp/CCDOSPY.el")),
    ("CTLau", include_str!("../../lisp/CTLau.el")),
    ("CTLau-b5", include_str!("../../lisp/CTLau-b5.el")),
    ("ETZY", include_str!("../../lisp/ETZY.el")),
    ("PY", include_str!("../../lisp/PY.el")),
    ("PY-b5", include_str!("../../lisp/PY-b5.el")),
    ("Punct", include_str!("../../lisp/Punct.el")),
    ("QJ", include_str!("../../lisp/QJ.el")),
    ("QJ-b5", include_str!("../../lisp/QJ-b5.el")),
    ("SW", include_str!("../../lisp/SW.el")),
    ("TONEPY", include_str!("../../lisp/TONEPY.el")),
    ("ZIRANMA", include_str!("../../lisp/ZIRANMA.el")),
    ("ZOZY", include_str!("../../lisp/ZOZY.el")),
    ("arabic", include_str!("../../lisp/arabic.el")),
    ("compose", include_str!("../../lisp/compose.el")),
    ("croatian", include_str!("../../lisp/croatian.el")),
    ("cyril-jis", include_str!("../../lisp/cyril-jis.el")),
    ("hangul", include_str!("../../lisp/hangul.el")),
    ("hanja", include_str!("../../lisp/hanja.el")),
    ("hanja-jis", include_str!("../../lisp/hanja-jis.el")),
    ("hanja3", include_str!("../../lisp/hanja3.el")),
    ("ipa", include_str!("../../lisp/ipa.el")),
    ("ipa-praat", include_str!("../../lisp/ipa-praat.el")),
    ("iroquoian", include_str!("../../lisp/iroquoian.el")),
    ("latin-alt", include_str!("../../lisp/latin-alt.el")),
    ("latin-ltx", include_str!("../../lisp/latin-ltx.el")),
    ("latin-post", include_str!("../../lisp/latin-post.el")),
    ("latin-pre", include_str!("../../lisp/latin-pre.el")),
    ("lrt", include_str!("../../lisp/lrt.el")),
    ("pakistan", include_str!("../../lisp/pakistan.el")),
    ("persian", include_str!("../../lisp/persian.el")),
    ("programmer-dvorak", include_str!("../../lisp/programmer-dvorak.el")),
    ("py-punct", include_str!("../../lisp/py-punct.el")),
    ("pypunct-b5", include_str!("../../lisp/pypunct-b5.el")),
    ("quick-b5", include_str!("../../lisp/quick-b5.el")),
    ("rfc1345", include_str!("../../lisp/rfc1345.el")),
    ("sami", include_str!("../../lisp/sami.el")),
    ("sgml-input", include_str!("../../lisp/sgml-input.el")),
    ("sisheng", include_str!("../../lisp/sisheng.el")),
    ("symbol-ksc", include_str!("../../lisp/symbol-ksc.el")),
    ("syriac", include_str!("../../lisp/syriac.el")),
    ("tamil-dvorak", include_str!("../../lisp/tamil-dvorak.el")),
    ("tifinagh", include_str!("../../lisp/tifinagh.el")),
    ("tsang-b5", include_str!("../../lisp/tsang-b5.el")),
    ("uni-input", include_str!("../../lisp/uni-input.el")),
    ("viqr", include_str!("../../lisp/viqr.el")),
    ("vntelex", include_str!("../../lisp/vntelex.el")),
    ("vnvni", include_str!("../../lisp/vnvni.el")),
    ("welsh", include_str!("../../lisp/welsh.el")),
    ("quail/burmese", include_str!("../../lisp/quail-burmese.el")),
    ("quail/cham", include_str!("../../lisp/quail-cham.el")),
    ("quail/cyrillic", include_str!("../../lisp/quail-cyrillic.el")),
    ("quail/czech", include_str!("../../lisp/quail-czech.el")),
    ("quail/emoji", include_str!("../../lisp/quail-emoji.el")),
    ("quail/georgian", include_str!("../../lisp/quail-georgian.el")),
    ("quail/greek", include_str!("../../lisp/quail-greek.el")),
    ("quail/hebrew", include_str!("../../lisp/quail-hebrew.el")),
    ("quail/indian", include_str!("../../lisp/quail-indian.el")),
    ("quail/indonesian", include_str!("../../lisp/quail-indonesian.el")),
    ("quail/japanese", include_str!("../../lisp/quail-japanese.el")),
    ("quail/lao", include_str!("../../lisp/quail-lao.el")),
    ("quail/misc-lang", include_str!("../../lisp/quail-misc-lang.el")),
    ("quail/philippine", include_str!("../../lisp/quail-philippine.el")),
    ("quail/slovak", include_str!("../../lisp/quail-slovak.el")),
    ("quail/thai", include_str!("../../lisp/quail-thai.el")),
    ("leim-list", include_str!("../../lisp/leim-list.el")),
    ("ja-dic", include_str!("../../lisp/ja-dic.el")),
    ("5x5", include_str!("../../lisp/5x5.el")),
    ("ange-ftp", include_str!("../../lisp/ange-ftp.el")),  // not in GNU tree (adapted/remacs-specific)
    ("auth-source-pass", include_str!("../../lisp/auth-source-pass.el")),
    ("battery", include_str!("../../lisp/battery.el")),
    ("bibtex", include_str!("../../lisp/bibtex.el")),
    ("button", include_str!("../../lisp/button.el")),  // not in GNU tree (adapted/remacs-specific)
    ("canlock", include_str!("../../lisp/canlock.el")),
    ("cedet-cscope", include_str!("../../lisp/cedet-cscope.el")),
    ("cedet-files", include_str!("../../lisp/cedet-files.el")),
    ("cedet-global", include_str!("../../lisp/cedet-global.el")),
    ("cedet-idutils", include_str!("../../lisp/cedet-idutils.el")),
    ("cedet", include_str!("../../lisp/cedet.el")),
    ("cl-compat", include_str!("../../lisp/cl-compat.el")),
    ("crisp", include_str!("../../lisp/crisp.el")),
    ("cus-load", include_str!("../../lisp/cus-load.el")),
    ("custom", include_str!("../../lisp/custom.el")),  // not in GNU tree (adapted/remacs-specific)
    ("data-debug", include_str!("../../lisp/data-debug.el")),
    ("deuglify", include_str!("../../lisp/deuglify.el")),
    ("dframe", include_str!("../../lisp/dframe.el")),
    ("diary-icalendar", include_str!("../../lisp/diary-icalendar.el")),  // not in GNU tree (adapted/remacs-specific)
    ("dictionary-connection", include_str!("../../lisp/dictionary-connection.el")),
    ("dictionary", include_str!("../../lisp/dictionary.el")),
    ("dired-loaddefs", include_str!("../../lisp/dired-loaddefs.el")),
    ("dnd", include_str!("../../lisp/dnd.el")),
    ("doc-view", include_str!("../../lisp/doc-view.el")),
    ("ebnf-abn", include_str!("../../lisp/ebnf-abn.el")),
    ("ebnf-bnf", include_str!("../../lisp/ebnf-bnf.el")),
    ("ebnf-dtd", include_str!("../../lisp/ebnf-dtd.el")),
    ("ebnf-ebx", include_str!("../../lisp/ebnf-ebx.el")),
    ("ebnf-iso", include_str!("../../lisp/ebnf-iso.el")),
    ("ebnf-otz", include_str!("../../lisp/ebnf-otz.el")),
    ("ebnf-yac", include_str!("../../lisp/ebnf-yac.el")),
    ("ebnf2ps", include_str!("../../lisp/ebnf2ps.el")),
    ("ede/auto", include_str!("../../lisp/ede-auto.el")),  // GNU ede/auto
    ("ede/autoconf-edit", include_str!("../../lisp/ede-autoconf-edit.el")),  // GNU ede/autoconf-edit
    ("ede/base", include_str!("../../lisp/ede-base.el")),  // GNU ede/base
    ("ede/config", include_str!("../../lisp/ede-config.el")),  // GNU ede/config
    ("ede/cpp-root", include_str!("../../lisp/ede-cpp-root.el")),  // GNU ede/cpp-root
    ("ede/custom", include_str!("../../lisp/ede-custom.el")),  // GNU ede/custom
    ("ede/detect", include_str!("../../lisp/ede-detect.el")),  // GNU ede/detect
    ("ede/dired", include_str!("../../lisp/ede-dired.el")),  // GNU ede/dired
    ("ede/emacs", include_str!("../../lisp/ede-emacs.el")),  // GNU ede/emacs
    ("ede/files", include_str!("../../lisp/ede-files.el")),  // GNU ede/files
    ("ede/generic", include_str!("../../lisp/ede-generic.el")),  // GNU ede/generic
    ("ede/linux", include_str!("../../lisp/ede-linux.el")),  // GNU ede/linux
    ("ede/loaddefs", include_str!("../../lisp/ede-loaddefs.el")),  // GNU ede/loaddefs
    ("ede/locate", include_str!("../../lisp/ede-locate.el")),  // GNU ede/locate
    ("ede/make", include_str!("../../lisp/ede-make.el")),  // GNU ede/make
    ("ede/pconf", include_str!("../../lisp/ede-pconf.el")),  // GNU ede/pconf
    ("ede/pmake", include_str!("../../lisp/ede-pmake.el")),  // GNU ede/pmake
    ("ede/proj-archive", include_str!("../../lisp/ede-proj-archive.el")),  // GNU ede/proj-archive
    ("ede/proj-aux", include_str!("../../lisp/ede-proj-aux.el")),  // GNU ede/proj-aux
    ("ede/proj-comp", include_str!("../../lisp/ede-proj-comp.el")),  // GNU ede/proj-comp
    ("ede/proj-elisp", include_str!("../../lisp/ede-proj-elisp.el")),  // GNU ede/proj-elisp
    ("ede/proj-info", include_str!("../../lisp/ede-proj-info.el")),  // GNU ede/proj-info
    ("ede/proj-misc", include_str!("../../lisp/ede-proj-misc.el")),  // GNU ede/proj-misc
    ("ede/proj-obj", include_str!("../../lisp/ede-proj-obj.el")),  // GNU ede/proj-obj
    ("ede/proj-prog", include_str!("../../lisp/ede-proj-prog.el")),  // GNU ede/proj-prog
    ("ede/proj-scheme", include_str!("../../lisp/ede-proj-scheme.el")),  // GNU ede/proj-scheme
    ("ede/proj-shared", include_str!("../../lisp/ede-proj-shared.el")),  // GNU ede/proj-shared
    ("ede/proj", include_str!("../../lisp/ede-proj.el")),  // GNU ede/proj
    ("ede/project-am", include_str!("../../lisp/ede-project-am.el")),  // GNU ede/project-am
    ("ede/shell", include_str!("../../lisp/ede-shell.el")),  // GNU ede/shell
    ("ede/simple", include_str!("../../lisp/ede-simple.el")),  // GNU ede/simple
    ("ede/source", include_str!("../../lisp/ede-source.el")),  // GNU ede/source
    ("ede/speedbar", include_str!("../../lisp/ede-speedbar.el")),  // GNU ede/speedbar
    ("ede/srecode", include_str!("../../lisp/ede-srecode.el")),  // GNU ede/srecode
    ("ede/system", include_str!("../../lisp/ede-system.el")),  // GNU ede/system
    ("ede/util", include_str!("../../lisp/ede-util.el")),  // GNU ede/util
    ("ede", include_str!("../../lisp/ede.el")),
    ("ediff-diff", include_str!("../../lisp/ediff-diff.el")),
    ("ediff-help", include_str!("../../lisp/ediff-help.el")),
    ("ediff-init", include_str!("../../lisp/ediff-init.el")),  // not in GNU tree (adapted/remacs-specific)
    ("ediff-merg", include_str!("../../lisp/ediff-merg.el")),
    ("ediff-mult", include_str!("../../lisp/ediff-mult.el")),
    ("ediff-ptch", include_str!("../../lisp/ediff-ptch.el")),
    ("ediff-util", include_str!("../../lisp/ediff-util.el")),
    ("ediff-vers", include_str!("../../lisp/ediff-vers.el")),
    ("ediff-wind", include_str!("../../lisp/ediff-wind.el")),
    ("ediff", include_str!("../../lisp/ediff.el")),
    ("edmacro", include_str!("../../lisp/edmacro.el")),  // not in GNU tree (adapted/remacs-specific)
    ("elp", include_str!("../../lisp/elp.el")),  // not in GNU tree (adapted/remacs-specific)
    ("em-alias", include_str!("../../lisp/em-alias.el")),
    ("em-banner", include_str!("../../lisp/em-banner.el")),
    ("em-basic", include_str!("../../lisp/em-basic.el")),
    ("em-cmpl", include_str!("../../lisp/em-cmpl.el")),
    ("em-dirs", include_str!("../../lisp/em-dirs.el")),
    ("em-elecslash", include_str!("../../lisp/em-elecslash.el")),  // not in GNU tree (adapted/remacs-specific)
    ("em-extpipe", include_str!("../../lisp/em-extpipe.el")),
    ("em-glob", include_str!("../../lisp/em-glob.el")),
    ("em-hist", include_str!("../../lisp/em-hist.el")),
    ("em-ls", include_str!("../../lisp/em-ls.el")),
    ("em-pred", include_str!("../../lisp/em-pred.el")),
    ("em-prompt", include_str!("../../lisp/em-prompt.el")),
    ("em-rebind", include_str!("../../lisp/em-rebind.el")),
    ("em-script", include_str!("../../lisp/em-script.el")),
    ("em-smart", include_str!("../../lisp/em-smart.el")),
    ("em-term", include_str!("../../lisp/em-term.el")),
    ("em-tramp", include_str!("../../lisp/em-tramp.el")),  // not in GNU tree (adapted/remacs-specific)
    ("em-unix", include_str!("../../lisp/em-unix.el")),
    ("em-xtra", include_str!("../../lisp/em-xtra.el")),
    ("emacs-news-mode", include_str!("../../lisp/emacs-news-mode.el")),
    ("epa-dired", include_str!("../../lisp/epa-dired.el")),
    ("epa-file", include_str!("../../lisp/epa-file.el")),
    ("epa-ks", include_str!("../../lisp/epa-ks.el")),
    ("epa-mail", include_str!("../../lisp/epa-mail.el")),
    ("esh-arg", include_str!("../../lisp/esh-arg.el")),
    ("esh-cmd", include_str!("../../lisp/esh-cmd.el")),
    ("esh-ext", include_str!("../../lisp/esh-ext.el")),
    ("esh-io", include_str!("../../lisp/esh-io.el")),
    ("esh-mode", include_str!("../../lisp/esh-mode.el")),  // not in GNU tree (adapted/remacs-specific)
    ("esh-module-loaddefs", include_str!("../../lisp/esh-module-loaddefs.el")),
    ("esh-module", include_str!("../../lisp/esh-module.el")),
    ("esh-opt", include_str!("../../lisp/esh-opt.el")),
    ("esh-proc", include_str!("../../lisp/esh-proc.el")),
    ("esh-util", include_str!("../../lisp/esh-util.el")),
    ("esh-var", include_str!("../../lisp/esh-var.el")),
    ("eshell", include_str!("../../lisp/eshell.el")),
    ("exif", include_str!("../../lisp/exif.el")),
    ("faces", include_str!("../../lisp/faces.el")),  // not in GNU tree (adapted/remacs-specific)
    ("ffap", include_str!("../../lisp/ffap.el")),
    ("files", include_str!("../../lisp/files.el")),  // not in GNU tree (adapted/remacs-specific)
    ("finder-inf", include_str!("../../lisp/finder-inf.el")),
    ("finder", include_str!("../../lisp/finder.el")),
    ("frame", include_str!("../../lisp/frame.el")),  // not in GNU tree (adapted/remacs-specific)
    ("gametree", include_str!("../../lisp/gametree.el")),
    ("generic-x", include_str!("../../lisp/generic-x.el")),
    ("gmm-utils", include_str!("../../lisp/gmm-utils.el")),
    ("gnus-agent", include_str!("../../lisp/gnus-agent.el")),
    ("gnus-art", include_str!("../../lisp/gnus-art.el")),
    ("gnus-async", include_str!("../../lisp/gnus-async.el")),
    ("gnus-bcklg", include_str!("../../lisp/gnus-bcklg.el")),
    ("gnus-bookmark", include_str!("../../lisp/gnus-bookmark.el")),
    ("gnus-cache", include_str!("../../lisp/gnus-cache.el")),
    ("gnus-cite", include_str!("../../lisp/gnus-cite.el")),
    ("gnus-cloud", include_str!("../../lisp/gnus-cloud.el")),
    ("gnus-cus", include_str!("../../lisp/gnus-cus.el")),
    ("gnus-delay", include_str!("../../lisp/gnus-delay.el")),
    ("gnus-demon", include_str!("../../lisp/gnus-demon.el")),
    ("gnus-diary", include_str!("../../lisp/gnus-diary.el")),
    ("gnus-dired", include_str!("../../lisp/gnus-dired.el")),
    ("gnus-draft", include_str!("../../lisp/gnus-draft.el")),
    ("gnus-dup", include_str!("../../lisp/gnus-dup.el")),
    ("gnus-eform", include_str!("../../lisp/gnus-eform.el")),
    ("gnus-fun", include_str!("../../lisp/gnus-fun.el")),
    ("gnus-gravatar", include_str!("../../lisp/gnus-gravatar.el")),
    ("gnus-group", include_str!("../../lisp/gnus-group.el")),
    ("gnus-html", include_str!("../../lisp/gnus-html.el")),
    ("gnus-icalendar", include_str!("../../lisp/gnus-icalendar.el")),
    ("gnus-int", include_str!("../../lisp/gnus-int.el")),
    ("gnus-kill", include_str!("../../lisp/gnus-kill.el")),
    ("gnus-logic", include_str!("../../lisp/gnus-logic.el")),
    ("gnus-mh", include_str!("../../lisp/gnus-mh.el")),
    ("gnus-ml", include_str!("../../lisp/gnus-ml.el")),
    ("gnus-mlspl", include_str!("../../lisp/gnus-mlspl.el")),
    ("gnus-msg", include_str!("../../lisp/gnus-msg.el")),
    ("gnus-notifications", include_str!("../../lisp/gnus-notifications.el")),
    ("gnus-picon", include_str!("../../lisp/gnus-picon.el")),
    ("gnus-range", include_str!("../../lisp/gnus-range.el")),
    ("gnus-registry", include_str!("../../lisp/gnus-registry.el")),
    ("gnus-rfc1843", include_str!("../../lisp/gnus-rfc1843.el")),
    ("gnus-rmail", include_str!("../../lisp/gnus-rmail.el")),
    ("gnus-salt", include_str!("../../lisp/gnus-salt.el")),
    ("gnus-score", include_str!("../../lisp/gnus-score.el")),
    ("gnus-search", include_str!("../../lisp/gnus-search.el")),
    ("gnus-sieve", include_str!("../../lisp/gnus-sieve.el")),
    ("gnus-spec", include_str!("../../lisp/gnus-spec.el")),
    ("gnus-srvr", include_str!("../../lisp/gnus-srvr.el")),
    ("gnus-start", include_str!("../../lisp/gnus-start.el")),
    ("gnus-sum", include_str!("../../lisp/gnus-sum.el")),
    ("gnus-topic", include_str!("../../lisp/gnus-topic.el")),
    ("gnus-undo", include_str!("../../lisp/gnus-undo.el")),
    ("gnus-util", include_str!("../../lisp/gnus-util.el")),
    ("gnus-uu", include_str!("../../lisp/gnus-uu.el")),
    ("gnus-vm", include_str!("../../lisp/gnus-vm.el")),
    ("gnus-win", include_str!("../../lisp/gnus-win.el")),
    ("gnus", include_str!("../../lisp/gnus.el")),
    ("gssapi", include_str!("../../lisp/gssapi.el")),
    ("handwrite", include_str!("../../lisp/handwrite.el")),
    ("hexl", include_str!("../../lisp/hexl.el")),  // not in GNU tree (adapted/remacs-specific)
    ("hilit-chg", include_str!("../../lisp/hilit-chg.el")),
    ("htmlfontify", include_str!("../../lisp/htmlfontify.el")),
    ("ibuf-ext", include_str!("../../lisp/ibuf-ext.el")),
    ("ibuffer-loaddefs", include_str!("../../lisp/ibuffer-loaddefs.el")),
    ("ibuffer", include_str!("../../lisp/ibuffer.el")),
    ("icalendar-ast", include_str!("../../lisp/icalendar-ast.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar-macs", include_str!("../../lisp/icalendar-macs.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar-mode", include_str!("../../lisp/icalendar-mode.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar-parser", include_str!("../../lisp/icalendar-parser.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar-recur", include_str!("../../lisp/icalendar-recur.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar-shortdoc", include_str!("../../lisp/icalendar-shortdoc.el")),
    ("icalendar-utils", include_str!("../../lisp/icalendar-utils.el")),  // not in GNU tree (adapted/remacs-specific)
    ("icalendar", include_str!("../../lisp/icalendar.el")),  // not in GNU tree (adapted/remacs-specific)
    ("image-mode", include_str!("../../lisp/image-mode.el")),
    ("inf-lisp", include_str!("../../lisp/inf-lisp.el")),
    ("info-look", include_str!("../../lisp/info-look.el")),
    ("info-xref", include_str!("../../lisp/info-xref.el")),
    ("info", include_str!("../../lisp/info.el")),
    ("informat", include_str!("../../lisp/informat.el")),
    ("isearch", include_str!("../../lisp/isearch.el")),  // not in GNU tree (adapted/remacs-specific)
    ("jsonrpc", include_str!("../../lisp/jsonrpc.el")),  // not in GNU tree (adapted/remacs-specific)
    ("kermit", include_str!("../../lisp/kermit.el")),
    ("loadup", include_str!("../../lisp/loadup.el")),
    ("log-edit", include_str!("../../lisp/log-edit.el")),
    ("log-view", include_str!("../../lisp/log-view.el")),
    ("lua-mode", include_str!("../../lisp/lua-mode.el")),
    ("mail-hist", include_str!("../../lisp/mail-hist.el")),
    ("mail-source", include_str!("../../lisp/mail-source.el")),
    ("mailabbrev", include_str!("../../lisp/mailabbrev.el")),
    ("mailalias", include_str!("../../lisp/mailalias.el")),
    ("makefile-edit", include_str!("../../lisp/makefile-edit.el")),
    ("menu-bar", include_str!("../../lisp/menu-bar.el")),  // not in GNU tree (adapted/remacs-specific)
    ("message", include_str!("../../lisp/message.el")),
    ("mm-archive", include_str!("../../lisp/mm-archive.el")),
    ("mm-bodies", include_str!("../../lisp/mm-bodies.el")),
    ("mm-decode", include_str!("../../lisp/mm-decode.el")),
    ("mm-encode", include_str!("../../lisp/mm-encode.el")),
    ("mm-extern", include_str!("../../lisp/mm-extern.el")),
    ("mm-partial", include_str!("../../lisp/mm-partial.el")),
    ("mm-url", include_str!("../../lisp/mm-url.el")),
    ("mm-uu", include_str!("../../lisp/mm-uu.el")),
    ("mm-view", include_str!("../../lisp/mm-view.el")),
    ("mml-sec", include_str!("../../lisp/mml-sec.el")),
    ("mml-smime", include_str!("../../lisp/mml-smime.el")),
    ("mml", include_str!("../../lisp/mml.el")),
    ("mml1991", include_str!("../../lisp/mml1991.el")),
    ("mml2015", include_str!("../../lisp/mml2015.el")),
    ("mode-local", include_str!("../../lisp/mode-local.el")),
    ("mpc", include_str!("../../lisp/mpc.el")),
    ("mwheel", include_str!("../../lisp/mwheel.el")),  // not in GNU tree (adapted/remacs-specific)
    ("newst-backend", include_str!("../../lisp/newst-backend.el")),
    ("newst-plainview", include_str!("../../lisp/newst-plainview.el")),
    ("newst-reader", include_str!("../../lisp/newst-reader.el")),
    ("newst-ticker", include_str!("../../lisp/newst-ticker.el")),
    ("newst-treeview", include_str!("../../lisp/newst-treeview.el")),
    ("newsticker", include_str!("../../lisp/newsticker.el")),
    ("nnagent", include_str!("../../lisp/nnagent.el")),
    ("nnatom", include_str!("../../lisp/nnatom.el")),
    ("nnbabyl", include_str!("../../lisp/nnbabyl.el")),
    ("nndiary", include_str!("../../lisp/nndiary.el")),
    ("nndir", include_str!("../../lisp/nndir.el")),
    ("nndoc", include_str!("../../lisp/nndoc.el")),
    ("nndraft", include_str!("../../lisp/nndraft.el")),
    ("nneething", include_str!("../../lisp/nneething.el")),
    ("nnfeed", include_str!("../../lisp/nnfeed.el")),
    ("nnfolder", include_str!("../../lisp/nnfolder.el")),
    ("nngateway", include_str!("../../lisp/nngateway.el")),
    ("nnheader", include_str!("../../lisp/nnheader.el")),
    ("nnimap", include_str!("../../lisp/nnimap.el")),
    ("nnmail", include_str!("../../lisp/nnmail.el")),
    ("nnmaildir", include_str!("../../lisp/nnmaildir.el")),
    ("nnmairix", include_str!("../../lisp/nnmairix.el")),
    ("nnmbox", include_str!("../../lisp/nnmbox.el")),
    ("nnmh", include_str!("../../lisp/nnmh.el")),
    ("nnml", include_str!("../../lisp/nnml.el")),
    ("nnnil", include_str!("../../lisp/nnnil.el")),
    ("nnoo", include_str!("../../lisp/nnoo.el")),
    ("nnregistry", include_str!("../../lisp/nnregistry.el")),
    ("nnrss", include_str!("../../lisp/nnrss.el")),
    ("nnselect", include_str!("../../lisp/nnselect.el")),
    ("nnspool", include_str!("../../lisp/nnspool.el")),
    ("nntp", include_str!("../../lisp/nntp.el")),
    ("nnvirtual", include_str!("../../lisp/nnvirtual.el")),
    ("nnweb", include_str!("../../lisp/nnweb.el")),
    ("notifications", include_str!("../../lisp/notifications.el")),
    ("ob-C", include_str!("../../lisp/ob-C.el")),
    ("ob-R", include_str!("../../lisp/ob-R.el")),
    ("ob-awk", include_str!("../../lisp/ob-awk.el")),
    ("ob-calc", include_str!("../../lisp/ob-calc.el")),
    ("ob-clojure", include_str!("../../lisp/ob-clojure.el")),
    ("ob-comint", include_str!("../../lisp/ob-comint.el")),
    ("ob-core", include_str!("../../lisp/ob-core.el")),
    ("ob-csharp", include_str!("../../lisp/ob-csharp.el")),
    ("ob-css", include_str!("../../lisp/ob-css.el")),
    ("ob-ditaa", include_str!("../../lisp/ob-ditaa.el")),
    ("ob-dot", include_str!("../../lisp/ob-dot.el")),
    ("ob-emacs-lisp", include_str!("../../lisp/ob-emacs-lisp.el")),
    ("ob-eshell", include_str!("../../lisp/ob-eshell.el")),
    ("ob-eval", include_str!("../../lisp/ob-eval.el")),
    ("ob-exp", include_str!("../../lisp/ob-exp.el")),
    ("ob-forth", include_str!("../../lisp/ob-forth.el")),
    ("ob-fortran", include_str!("../../lisp/ob-fortran.el")),
    ("ob-gnuplot", include_str!("../../lisp/ob-gnuplot.el")),
    ("ob-groovy", include_str!("../../lisp/ob-groovy.el")),
    ("ob-haskell", include_str!("../../lisp/ob-haskell.el")),
    ("ob-java", include_str!("../../lisp/ob-java.el")),
    ("ob-js", include_str!("../../lisp/ob-js.el")),
    ("ob-julia", include_str!("../../lisp/ob-julia.el")),
    ("ob-latex", include_str!("../../lisp/ob-latex.el")),
    ("ob-lilypond", include_str!("../../lisp/ob-lilypond.el")),
    ("ob-lisp", include_str!("../../lisp/ob-lisp.el")),
    ("ob-lob", include_str!("../../lisp/ob-lob.el")),
    ("ob-lua", include_str!("../../lisp/ob-lua.el")),
    ("ob-makefile", include_str!("../../lisp/ob-makefile.el")),
    ("ob-matlab", include_str!("../../lisp/ob-matlab.el")),
    ("ob-maxima", include_str!("../../lisp/ob-maxima.el")),
    ("ob-ocaml", include_str!("../../lisp/ob-ocaml.el")),
    ("ob-octave", include_str!("../../lisp/ob-octave.el")),
    ("ob-org", include_str!("../../lisp/ob-org.el")),
    ("ob-perl", include_str!("../../lisp/ob-perl.el")),
    ("ob-plantuml", include_str!("../../lisp/ob-plantuml.el")),
    ("ob-processing", include_str!("../../lisp/ob-processing.el")),
    ("ob-python", include_str!("../../lisp/ob-python.el")),
    ("ob-ref", include_str!("../../lisp/ob-ref.el")),
    ("ob-ruby", include_str!("../../lisp/ob-ruby.el")),
    ("ob-sass", include_str!("../../lisp/ob-sass.el")),
    ("ob-scheme", include_str!("../../lisp/ob-scheme.el")),
    ("ob-screen", include_str!("../../lisp/ob-screen.el")),
    ("ob-sed", include_str!("../../lisp/ob-sed.el")),
    ("ob-shell", include_str!("../../lisp/ob-shell.el")),
    ("ob-sql", include_str!("../../lisp/ob-sql.el")),
    ("ob-sqlite", include_str!("../../lisp/ob-sqlite.el")),
    ("ob-table", include_str!("../../lisp/ob-table.el")),
    ("ob-tangle", include_str!("../../lisp/ob-tangle.el")),
    ("ob", include_str!("../../lisp/ob.el")),
    ("oc-basic", include_str!("../../lisp/oc-basic.el")),
    ("oc-biblatex", include_str!("../../lisp/oc-biblatex.el")),
    ("oc-bibtex", include_str!("../../lisp/oc-bibtex.el")),
    ("oc-csl", include_str!("../../lisp/oc-csl.el")),
    ("oc-natbib", include_str!("../../lisp/oc-natbib.el")),
    ("oc", include_str!("../../lisp/oc.el")),
    ("ol-bbdb", include_str!("../../lisp/ol-bbdb.el")),
    ("ol-bibtex", include_str!("../../lisp/ol-bibtex.el")),
    ("ol-docview", include_str!("../../lisp/ol-docview.el")),
    ("ol-doi", include_str!("../../lisp/ol-doi.el")),
    ("ol-eshell", include_str!("../../lisp/ol-eshell.el")),
    ("ol-eww", include_str!("../../lisp/ol-eww.el")),
    ("ol-gnus", include_str!("../../lisp/ol-gnus.el")),
    ("ol-info", include_str!("../../lisp/ol-info.el")),
    ("ol-irc", include_str!("../../lisp/ol-irc.el")),
    ("ol-man", include_str!("../../lisp/ol-man.el")),
    ("ol-mhe", include_str!("../../lisp/ol-mhe.el")),
    ("ol-rmail", include_str!("../../lisp/ol-rmail.el")),
    ("ol-w3m", include_str!("../../lisp/ol-w3m.el")),
    ("ol", include_str!("../../lisp/ol.el")),
    ("org-agenda", include_str!("../../lisp/org-agenda.el")),
    ("org-archive", include_str!("../../lisp/org-archive.el")),
    ("org-attach-git", include_str!("../../lisp/org-attach-git.el")),
    ("org-attach", include_str!("../../lisp/org-attach.el")),
    ("org-capture", include_str!("../../lisp/org-capture.el")),
    ("org-clock", include_str!("../../lisp/org-clock.el")),
    ("org-colview", include_str!("../../lisp/org-colview.el")),
    ("org-compat", include_str!("../../lisp/org-compat.el")),
    ("org-crypt", include_str!("../../lisp/org-crypt.el")),
    ("org-ctags", include_str!("../../lisp/org-ctags.el")),
    ("org-cycle", include_str!("../../lisp/org-cycle.el")),
    ("org-datetree", include_str!("../../lisp/org-datetree.el")),
    ("org-duration", include_str!("../../lisp/org-duration.el")),
    ("org-element-ast", include_str!("../../lisp/org-element-ast.el")),  // not in GNU tree (adapted/remacs-specific)
    ("org-element", include_str!("../../lisp/org-element.el")),
    ("org-entities", include_str!("../../lisp/org-entities.el")),
    ("org-faces", include_str!("../../lisp/org-faces.el")),
    ("org-feed", include_str!("../../lisp/org-feed.el")),
    ("org-fold-core", include_str!("../../lisp/org-fold-core.el")),
    ("org-fold", include_str!("../../lisp/org-fold.el")),
    ("org-footnote", include_str!("../../lisp/org-footnote.el")),
    ("org-goto", include_str!("../../lisp/org-goto.el")),
    ("org-habit", include_str!("../../lisp/org-habit.el")),
    ("org-id", include_str!("../../lisp/org-id.el")),
    ("org-indent", include_str!("../../lisp/org-indent.el")),
    ("org-inlinetask", include_str!("../../lisp/org-inlinetask.el")),
    ("org-keys", include_str!("../../lisp/org-keys.el")),
    ("org-lint", include_str!("../../lisp/org-lint.el")),
    ("org-list", include_str!("../../lisp/org-list.el")),
    ("org-loaddefs", include_str!("../../lisp/org-loaddefs.el")),
    ("org-macro", include_str!("../../lisp/org-macro.el")),
    ("org-macs", include_str!("../../lisp/org-macs.el")),
    ("org-mobile", include_str!("../../lisp/org-mobile.el")),
    ("org-mouse", include_str!("../../lisp/org-mouse.el")),
    ("org-num", include_str!("../../lisp/org-num.el")),
    ("org-pcomplete", include_str!("../../lisp/org-pcomplete.el")),
    ("org-persist", include_str!("../../lisp/org-persist.el")),
    ("org-plot", include_str!("../../lisp/org-plot.el")),
    ("org-protocol", include_str!("../../lisp/org-protocol.el")),
    ("org-refile", include_str!("../../lisp/org-refile.el")),
    ("org-src", include_str!("../../lisp/org-src.el")),
    ("org-table", include_str!("../../lisp/org-table.el")),
    ("org-tempo", include_str!("../../lisp/org-tempo.el")),
    ("org-timer", include_str!("../../lisp/org-timer.el")),
    ("org-version", include_str!("../../lisp/org-version.el")),
    ("org", include_str!("../../lisp/org.el")),
    ("ox-ascii", include_str!("../../lisp/ox-ascii.el")),
    ("ox-beamer", include_str!("../../lisp/ox-beamer.el")),
    ("ox-html", include_str!("../../lisp/ox-html.el")),
    ("ox-icalendar", include_str!("../../lisp/ox-icalendar.el")),
    ("ox-koma-letter", include_str!("../../lisp/ox-koma-letter.el")),
    ("ox-latex", include_str!("../../lisp/ox-latex.el")),
    ("ox-man", include_str!("../../lisp/ox-man.el")),
    ("ox-md", include_str!("../../lisp/ox-md.el")),
    ("ox-odt", include_str!("../../lisp/ox-odt.el")),
    ("ox-org", include_str!("../../lisp/ox-org.el")),
    ("ox-publish", include_str!("../../lisp/ox-publish.el")),
    ("ox-texinfo", include_str!("../../lisp/ox-texinfo.el")),
    ("ox", include_str!("../../lisp/ox.el")),
    ("pcmpl-cvs", include_str!("../../lisp/pcmpl-cvs.el")),
    ("pcmpl-git", include_str!("../../lisp/pcmpl-git.el")),
    ("pcmpl-gnu", include_str!("../../lisp/pcmpl-gnu.el")),
    ("pcmpl-linux", include_str!("../../lisp/pcmpl-linux.el")),
    ("pcmpl-rpm", include_str!("../../lisp/pcmpl-rpm.el")),
    ("pcmpl-unix", include_str!("../../lisp/pcmpl-unix.el")),
    ("pcmpl-x", include_str!("../../lisp/pcmpl-x.el")),
    ("peg", include_str!("../../lisp/peg.el")),  // not in GNU tree (adapted/remacs-specific)
    ("pgtk-dnd", include_str!("../../lisp/pgtk-dnd.el")),
    ("ps-bdf", include_str!("../../lisp/ps-bdf.el")),
    ("ps-mode", include_str!("../../lisp/ps-mode.el")),
    ("ps-mule", include_str!("../../lisp/ps-mule.el")),
    ("refer", include_str!("../../lisp/refer.el")),
    ("remacs-compat", include_str!("../../lisp/remacs-compat.el")),  // not in GNU tree (adapted/remacs-specific)
    ("score-mode", include_str!("../../lisp/score-mode.el")),
    ("semantic/analyze/complete", include_str!("../../lisp/semantic-analyze-complete.el")),  // GNU semantic/analyze/complete
    ("semantic/analyze/debug", include_str!("../../lisp/semantic-analyze-debug.el")),  // GNU semantic/analyze/debug
    ("semantic/analyze/fcn", include_str!("../../lisp/semantic-analyze-fcn.el")),  // GNU semantic/analyze/fcn
    ("semantic/analyze/refs", include_str!("../../lisp/semantic-analyze-refs.el")),  // GNU semantic/analyze/refs
    ("semantic/analyze", include_str!("../../lisp/semantic-analyze.el")),  // GNU semantic/analyze
    ("semantic/bovine/c-by", include_str!("../../lisp/semantic-bovine-c-by.el")),  // GNU semantic/bovine/c-by
    ("semantic/bovine/c", include_str!("../../lisp/semantic-bovine-c.el")),  // GNU semantic/bovine/c
    ("semantic/bovine/debug", include_str!("../../lisp/semantic-bovine-debug.el")),  // GNU semantic/bovine/debug
    ("semantic/bovine/el", include_str!("../../lisp/semantic-bovine-el.el")),  // GNU semantic/bovine/el
    ("semantic/bovine/gcc", include_str!("../../lisp/semantic-bovine-gcc.el")),  // GNU semantic/bovine/gcc
    ("semantic/bovine/grammar", include_str!("../../lisp/semantic-bovine-grammar.el")),  // GNU semantic/bovine/grammar
    ("semantic/bovine/make-by", include_str!("../../lisp/semantic-bovine-make-by.el")),  // GNU semantic/bovine/make-by
    ("semantic/bovine/make", include_str!("../../lisp/semantic-bovine-make.el")),  // GNU semantic/bovine/make
    ("semantic/bovine/scm-by", include_str!("../../lisp/semantic-bovine-scm-by.el")),  // GNU semantic/bovine/scm-by
    ("semantic/bovine/scm", include_str!("../../lisp/semantic-bovine-scm.el")),  // GNU semantic/bovine/scm
    ("semantic/bovine", include_str!("../../lisp/semantic-bovine.el")),  // GNU semantic/bovine
    ("semantic/chart", include_str!("../../lisp/semantic-chart.el")),  // GNU semantic/chart
    ("semantic/complete", include_str!("../../lisp/semantic-complete.el")),  // GNU semantic/complete
    ("semantic/ctxt", include_str!("../../lisp/semantic-ctxt.el")),  // GNU semantic/ctxt
    ("semantic/db-debug", include_str!("../../lisp/semantic-db-debug.el")),  // GNU semantic/db-debug
    ("semantic/db-ebrowse", include_str!("../../lisp/semantic-db-ebrowse.el")),  // GNU semantic/db-ebrowse
    ("semantic/db-el", include_str!("../../lisp/semantic-db-el.el")),  // GNU semantic/db-el
    ("semantic/db-file", include_str!("../../lisp/semantic-db-file.el")),  // GNU semantic/db-file
    ("semantic/db-find", include_str!("../../lisp/semantic-db-find.el")),  // GNU semantic/db-find
    ("semantic/db-global", include_str!("../../lisp/semantic-db-global.el")),  // GNU semantic/db-global
    ("semantic/db-javascript", include_str!("../../lisp/semantic-db-javascript.el")),  // GNU semantic/db-javascript
    ("semantic/db-mode", include_str!("../../lisp/semantic-db-mode.el")),  // GNU semantic/db-mode
    ("semantic/db-ref", include_str!("../../lisp/semantic-db-ref.el")),  // GNU semantic/db-ref
    ("semantic/db-typecache", include_str!("../../lisp/semantic-db-typecache.el")),  // GNU semantic/db-typecache
    ("semantic/db", include_str!("../../lisp/semantic-db.el")),  // GNU semantic/db
    ("semantic/debug", include_str!("../../lisp/semantic-debug.el")),  // GNU semantic/debug
    ("semantic/decorate/include", include_str!("../../lisp/semantic-decorate-include.el")),  // GNU semantic/decorate/include
    ("semantic/decorate/mode", include_str!("../../lisp/semantic-decorate-mode.el")),  // GNU semantic/decorate/mode
    ("semantic/decorate", include_str!("../../lisp/semantic-decorate.el")),  // GNU semantic/decorate
    ("semantic/dep", include_str!("../../lisp/semantic-dep.el")),  // GNU semantic/dep
    ("semantic/doc", include_str!("../../lisp/semantic-doc.el")),  // GNU semantic/doc
    ("semantic/ede-grammar", include_str!("../../lisp/semantic-ede-grammar.el")),  // GNU semantic/ede-grammar
    ("semantic/edit", include_str!("../../lisp/semantic-edit.el")),  // GNU semantic/edit
    ("semantic/find", include_str!("../../lisp/semantic-find.el")),  // GNU semantic/find
    ("semantic/format", include_str!("../../lisp/semantic-format.el")),  // GNU semantic/format
    ("semantic/fw", include_str!("../../lisp/semantic-fw.el")),  // GNU semantic/fw
    ("semantic/grammar-wy", include_str!("../../lisp/semantic-grammar-wy.el")),  // GNU semantic/grammar-wy
    ("semantic/grammar", include_str!("../../lisp/semantic-grammar.el")),  // GNU semantic/grammar
    ("semantic/grm-wy-boot", include_str!("../../lisp/semantic-grm-wy-boot.el")),  // GNU semantic/grm-wy-boot
    ("semantic/html", include_str!("../../lisp/semantic-html.el")),  // GNU semantic/html
    ("semantic/ia-sb", include_str!("../../lisp/semantic-ia-sb.el")),  // GNU semantic/ia-sb
    ("semantic/ia", include_str!("../../lisp/semantic-ia.el")),  // GNU semantic/ia
    ("semantic/idle", include_str!("../../lisp/semantic-idle.el")),  // GNU semantic/idle
    ("semantic/imenu", include_str!("../../lisp/semantic-imenu.el")),  // GNU semantic/imenu
    ("semantic/java", include_str!("../../lisp/semantic-java.el")),  // GNU semantic/java
    ("semantic/lex-spp", include_str!("../../lisp/semantic-lex-spp.el")),  // GNU semantic/lex-spp
    ("semantic/lex", include_str!("../../lisp/semantic-lex.el")),  // GNU semantic/lex
    ("semantic/loaddefs", include_str!("../../lisp/semantic-loaddefs.el")),  // GNU semantic/loaddefs
    ("semantic/mru-bookmark", include_str!("../../lisp/semantic-mru-bookmark.el")),  // GNU semantic/mru-bookmark
    ("semantic/sb", include_str!("../../lisp/semantic-sb.el")),  // GNU semantic/sb
    ("semantic/scope", include_str!("../../lisp/semantic-scope.el")),  // GNU semantic/scope
    ("semantic/senator", include_str!("../../lisp/semantic-senator.el")),  // GNU semantic/senator
    ("semantic/sort", include_str!("../../lisp/semantic-sort.el")),  // GNU semantic/sort
    ("semantic/symref/cscope", include_str!("../../lisp/semantic-symref-cscope.el")),  // GNU semantic/symref/cscope
    ("semantic/symref/filter", include_str!("../../lisp/semantic-symref-filter.el")),  // GNU semantic/symref/filter
    ("semantic/symref/global", include_str!("../../lisp/semantic-symref-global.el")),  // GNU semantic/symref/global
    ("semantic/symref/grep", include_str!("../../lisp/semantic-symref-grep.el")),  // GNU semantic/symref/grep
    ("semantic/symref/idutils", include_str!("../../lisp/semantic-symref-idutils.el")),  // GNU semantic/symref/idutils
    ("semantic/symref/list", include_str!("../../lisp/semantic-symref-list.el")),  // GNU semantic/symref/list
    ("semantic/symref", include_str!("../../lisp/semantic-symref.el")),  // GNU semantic/symref
    ("semantic/tag-file", include_str!("../../lisp/semantic-tag-file.el")),  // GNU semantic/tag-file
    ("semantic/tag-ls", include_str!("../../lisp/semantic-tag-ls.el")),  // GNU semantic/tag-ls
    ("semantic/tag-write", include_str!("../../lisp/semantic-tag-write.el")),  // GNU semantic/tag-write
    ("semantic/tag", include_str!("../../lisp/semantic-tag.el")),  // GNU semantic/tag
    ("semantic/texi", include_str!("../../lisp/semantic-texi.el")),  // GNU semantic/texi
    ("semantic/util-modes", include_str!("../../lisp/semantic-util-modes.el")),  // GNU semantic/util-modes
    ("semantic/util", include_str!("../../lisp/semantic-util.el")),  // GNU semantic/util
    ("semantic/wisent/comp", include_str!("../../lisp/semantic-wisent-comp.el")),  // GNU semantic/wisent/comp
    ("semantic/wisent/grammar", include_str!("../../lisp/semantic-wisent-grammar.el")),  // GNU semantic/wisent/grammar
    ("semantic/wisent/java-tags", include_str!("../../lisp/semantic-wisent-java-tags.el")),  // GNU semantic/wisent/java-tags
    ("semantic/wisent/javascript", include_str!("../../lisp/semantic-wisent-javascript.el")),  // GNU semantic/wisent/javascript
    ("semantic/wisent/javat-wy", include_str!("../../lisp/semantic-wisent-javat-wy.el")),  // GNU semantic/wisent/javat-wy
    ("semantic/wisent/js-wy", include_str!("../../lisp/semantic-wisent-js-wy.el")),  // GNU semantic/wisent/js-wy
    ("semantic/wisent/python-wy", include_str!("../../lisp/semantic-wisent-python-wy.el")),  // GNU semantic/wisent/python-wy
    ("semantic/wisent/python", include_str!("../../lisp/semantic-wisent-python.el")),  // GNU semantic/wisent/python
    ("semantic/wisent/wisent", include_str!("../../lisp/semantic-wisent-wisent.el")),  // GNU semantic/wisent/wisent
    ("semantic/wisent", include_str!("../../lisp/semantic-wisent.el")),  // GNU semantic/wisent
    ("semantic", include_str!("../../lisp/semantic.el")),
    ("seq", include_str!("../../lisp/seq.el")),  // not in GNU tree (adapted/remacs-specific)
    ("ses", include_str!("../../lisp/ses.el")),
    ("shadowfile", include_str!("../../lisp/shadowfile.el")),
    ("simple", include_str!("../../lisp/simple.el")),  // not in GNU tree (adapted/remacs-specific)
    ("smiley", include_str!("../../lisp/smiley.el")),
    ("smime", include_str!("../../lisp/smime.el")),
    ("smtpmail", include_str!("../../lisp/smtpmail.el")),
    ("spam-report", include_str!("../../lisp/spam-report.el")),
    ("spam-stat", include_str!("../../lisp/spam-stat.el")),
    ("spam-wash", include_str!("../../lisp/spam-wash.el")),
    ("spam", include_str!("../../lisp/spam.el")),
    ("speedbar", include_str!("../../lisp/speedbar.el")),
    ("srecode/args", include_str!("../../lisp/srecode-args.el")),  // GNU srecode/args
    ("srecode/compile", include_str!("../../lisp/srecode-compile.el")),  // GNU srecode/compile
    ("srecode/cpp", include_str!("../../lisp/srecode-cpp.el")),  // GNU srecode/cpp
    ("srecode/ctxt", include_str!("../../lisp/srecode-ctxt.el")),  // GNU srecode/ctxt
    ("srecode/dictionary", include_str!("../../lisp/srecode-dictionary.el")),  // GNU srecode/dictionary
    ("srecode/document", include_str!("../../lisp/srecode-document.el")),  // GNU srecode/document
    ("srecode/el", include_str!("../../lisp/srecode-el.el")),  // GNU srecode/el
    ("srecode/expandproto", include_str!("../../lisp/srecode-expandproto.el")),  // GNU srecode/expandproto
    ("srecode/extract", include_str!("../../lisp/srecode-extract.el")),  // GNU srecode/extract
    ("srecode/fields", include_str!("../../lisp/srecode-fields.el")),  // GNU srecode/fields
    ("srecode/filters", include_str!("../../lisp/srecode-filters.el")),  // GNU srecode/filters
    ("srecode/find", include_str!("../../lisp/srecode-find.el")),  // GNU srecode/find
    ("srecode/getset", include_str!("../../lisp/srecode-getset.el")),  // GNU srecode/getset
    ("srecode/insert", include_str!("../../lisp/srecode-insert.el")),  // GNU srecode/insert
    ("srecode/java", include_str!("../../lisp/srecode-java.el")),  // GNU srecode/java
    ("srecode/loaddefs", include_str!("../../lisp/srecode-loaddefs.el")),  // GNU srecode/loaddefs
    ("srecode/map", include_str!("../../lisp/srecode-map.el")),  // GNU srecode/map
    ("srecode/mode", include_str!("../../lisp/srecode-mode.el")),  // GNU srecode/mode
    ("srecode/semantic", include_str!("../../lisp/srecode-semantic.el")),  // GNU srecode/semantic
    ("srecode/srt-mode", include_str!("../../lisp/srecode-srt-mode.el")),  // GNU srecode/srt-mode
    ("srecode/srt-wy", include_str!("../../lisp/srecode-srt-wy.el")),  // GNU srecode/srt-wy
    ("srecode/srt", include_str!("../../lisp/srecode-srt.el")),  // GNU srecode/srt
    ("srecode/table", include_str!("../../lisp/srecode-table.el")),  // GNU srecode/table
    ("srecode/template", include_str!("../../lisp/srecode-template.el")),  // GNU srecode/template
    ("srecode/texi", include_str!("../../lisp/srecode-texi.el")),  // GNU srecode/texi
    ("srecode", include_str!("../../lisp/srecode.el")),
    ("subdirs", include_str!("../../lisp/subdirs.el")),
    ("subr", include_str!("../../lisp/subr.el")),  // not in GNU tree (adapted/remacs-specific)
    ("system-sleep", include_str!("../../lisp/system-sleep.el")),
    ("system-taskbar", include_str!("../../lisp/system-taskbar.el")),
    ("term", include_str!("../../lisp/term.el")),  // not in GNU tree (adapted/remacs-specific)
    ("texinfmt", include_str!("../../lisp/texinfmt.el")),
    ("texnfo-upd", include_str!("../../lisp/texnfo-upd.el")),
    ("theme-loaddefs", include_str!("../../lisp/theme-loaddefs.el")),
    ("transient", include_str!("../../lisp/transient.el")),
    ("treesit-x", include_str!("../../lisp/treesit-x.el")),
    ("treesit", include_str!("../../lisp/treesit.el")),
    ("url-auth", include_str!("../../lisp/url-auth.el")),
    ("url-cache", include_str!("../../lisp/url-cache.el")),
    ("url-cid", include_str!("../../lisp/url-cid.el")),
    ("url-cookie", include_str!("../../lisp/url-cookie.el")),
    ("url-dav", include_str!("../../lisp/url-dav.el")),
    ("url-domsuf", include_str!("../../lisp/url-domsuf.el")),
    ("url-expand", include_str!("../../lisp/url-expand.el")),
    ("url-file", include_str!("../../lisp/url-file.el")),
    ("url-ftp", include_str!("../../lisp/url-ftp.el")),
    ("url-future", include_str!("../../lisp/url-future.el")),
    ("url-gw", include_str!("../../lisp/url-gw.el")),
    ("url-handlers", include_str!("../../lisp/url-handlers.el")),  // not in GNU tree (adapted/remacs-specific)
    ("url-history", include_str!("../../lisp/url-history.el")),
    ("url-http", include_str!("../../lisp/url-http.el")),
    ("url-imap", include_str!("../../lisp/url-imap.el")),
    ("url-irc", include_str!("../../lisp/url-irc.el")),
    ("url-ldap", include_str!("../../lisp/url-ldap.el")),
    ("url-mailto", include_str!("../../lisp/url-mailto.el")),
    ("url-methods", include_str!("../../lisp/url-methods.el")),
    ("url-misc", include_str!("../../lisp/url-misc.el")),
    ("url-news", include_str!("../../lisp/url-news.el")),
    ("url-nfs", include_str!("../../lisp/url-nfs.el")),
    ("url-parse", include_str!("../../lisp/url-parse.el")),
    ("url-privacy", include_str!("../../lisp/url-privacy.el")),
    ("url-proxy", include_str!("../../lisp/url-proxy.el")),
    ("url-queue", include_str!("../../lisp/url-queue.el")),
    ("url-tramp", include_str!("../../lisp/url-tramp.el")),
    ("url-util", include_str!("../../lisp/url-util.el")),
    ("url-vars", include_str!("../../lisp/url-vars.el")),  // not in GNU tree (adapted/remacs-specific)
    ("url", include_str!("../../lisp/url.el")),  // not in GNU tree (adapted/remacs-specific)
    ("window-tool-bar", include_str!("../../lisp/window-tool-bar.el")),
    ("xwidget", include_str!("../../lisp/xwidget.el")),
];

/// Embedded source for library NAME (with or without .el/.elc suffix).
pub(crate) fn embedded(name: &str) -> Option<&'static str> {
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    EMBEDDED_LISP
        .iter()
        .find(|(n, _)| *n == stem)
        .or_else(|| {
            // Colliding basenames are bundled as SUBDIR-BASE.el, so
            // "quail/burmese" resolves to the quail input method,
            // not language/burmese.
            stem.rsplit_once('/').and_then(|(dir, base)| {
                let flat = format!("{dir}-{base}");
                EMBEDDED_LISP.iter().find(|(n, _)| *n == flat)
            })
        })
        .or_else(|| {
            // Autoload cells carry GNU's subdirectory paths
            // ("progmodes/f90"); our lisp/ tree is flat, so fall
            // back to the basename.
            stem.rsplit_once('/')
                .and_then(|(_, base)| EMBEDDED_LISP.iter().find(|(n, _)| *n == base))
        })
        .map(|(_, src)| *src)
}

/// Read the file at PATH and evaluate all forms in it.
/// Binds `load-file-name` and `load-in-progress` like Emacs `load`.
pub fn eval_file(i: &mut Interp, path: &str) -> EvalResult {
    eval_file_lex(i, path, false)
}

/// `--script FILE`: like `load`, but `lexical-binding' is forced on —
/// GNU's `command-line--load-script' does `setq-local lexical-binding t'
/// before eval-buffer, independent of any file cookie.
pub fn eval_file_script(i: &mut Interp, path: &str) -> EvalResult {
    eval_file_lex(i, path, true)
}

fn eval_file_lex(i: &mut Interp, path: &str, force_lex: bool) -> EvalResult {
    eval_file_lex_dumped(i, path, force_lex, false)
}

fn eval_file_lex_dumped(
    i: &mut Interp,
    path: &str,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            // GNU signals `file-missing' for ENOENT, `file-error' for
            // other failures, with data (FORMAT REASON PATH).
            let sym = if e.kind() == std::io::ErrorKind::NotFound {
                i.intern("file-missing")
            } else {
                crate::lisp::sym::FILE_ERROR
            };
            return Err(i.signal_data(
                sym,
                vec![
                    Value::string("Opening input file"),
                    Value::string(e.to_string()),
                    Value::string(path),
                ],
            ));
        }
    };
    // Track load-file-name / load-in-progress like Emacs does.  GNU
    // binds `load-file-name' to the filename `openp' located — the
    // tried path itself, not its truename.
    eval_src_opts(i, path, &src, force_lex, dumped_like)
}

/// Evaluate SRC as if loaded from file FILE (binds load-file-name,
/// load-in-progress, lexical-binding cookie; runs after-load hooks).
fn eval_src(i: &mut Interp, file: &str, src: &str, force_lex: bool) -> EvalResult {
    eval_src_opts(i, file, src, force_lex, false)
}

fn eval_src_opts(
    i: &mut Interp,
    file: &str,
    src: &str,
    force_lex: bool,
    dumped_like: bool,
) -> EvalResult {
    let lfn = i.intern("load-file-name");
    let lip = i.intern("load-in-progress");
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    i.specbind(lfn, Value::string(file))?;
    i.specbind(lip, Value::t())?;
    // Emacs: `load' honors a `lexical-binding' file cookie on the first
    // line (or the second, after a `#!' line); absent → dynamic eval.
    let lex_id = i.intern("lexical-binding");
    let lex_on = force_lex || file_lexical_binding(&src);
    i.specbind(lex_id, if lex_on { Value::t() } else { Value::Nil })?;
    // GNU's `readevalloop' specbinds `internal-interpreter-environment'
    // to `(t)' for lexical files (nil for dynamic) for the file's whole
    // dynamic extent: toplevel bare `defvar's become file-scoped special
    // declarations that unwind when the load finishes.
    let saved_lexenv = std::mem::replace(
        &mut i.lexenv,
        if lex_on {
            crate::lisp::eval::lexenv_root()
        } else {
            None
        },
    );
    // GNU's `internal--get-default-lexical-binding': a non-empty file
    // with no cookie gets a `files missing-lexbind-cookie' warning
    // before its forms run.  (--script forces lexical, so no warning.)
    if !lex_on && !src.is_empty() && !file.starts_with("builtin:") {
        let ty = Value::list(vec![
            Value::Sym(i.intern("files")),
            Value::Sym(i.intern("missing-lexbind-cookie")),
            Value::string(file),
        ]);
        let msg = Value::string(format!(
            "Missing \u{2018}lexical-binding\u{2019} cookie in {file:?}.\n\
             You can add one with \u{2018}M-x elisp-enable-lexical-binding RET\u{2019}.\n\
             See \u{2018}(elisp)Selecting Lisp Dialect\u{2019} and \
             \u{2018}(elisp)Converting to Lexical Binding\u{2019}\n\
             for more information."
        ));
        let dw = Value::Sym(i.intern("display-warning"));
        let lvl = Value::Sym(i.intern(":warning"));
        if i.apply(&dw, vec![ty, msg, lvl]).is_err() {
            // GNU's fallback when `display-warning' can't run yet.
            i.message(&format!(
                "Missing \u{2018}lexical-binding\u{2019} cookie in {file:?}"
            ));
        }
    }

    // Loading `icons' defines the `icon'/`icon-button' faces (GNU's
    // icons.el does this via defface).
    if !i.face_table.iter().any(|(n, _)| n == "icon") {
        i.face_table.push(("icon".to_string(), Value::Nil));
        i.face_table.push(("icon-button".to_string(), Value::Nil));
    }
    // Keep the `features' variable in sync with `i.features', but
    // never clobber a live dynamic binding: Gnus's
    // `(dlet ((features (cons 'gnus-group features))) (require ...))'
    // cycle-breaker relies on the binding surviving into nested
    // loads, so only append entries that are actually missing.
    let fid = i.intern("features");
    let cur = i.symbol_value(fid);
    let mut items = match &cur {
        Value::Cons(_) | Value::Nil => cur.list_to_vec().unwrap_or_default(),
        _ => vec![],
    };
    let mut missing: Vec<Value> = Vec::new();
    for sid in i.features.clone() {
        let sym_v = Value::Sym(sid);
        if !items.iter().any(|v| matches!(v, Value::Sym(s) if *s == sid)) {
            missing.push(sym_v);
        }
    }
    if !missing.is_empty() {
        items.extend(missing);
        i.obarray.symbol_mut(fid).value = Value::list(items);
    }
    i.specbind(cll, Value::Nil)?;
    // Embedded (`builtin:') libraries play the role of GNU's dumped
    // .elc files: functions defined while they load keep
    // `dumped_doc' docstring semantics.
    let was_dumped = i.loading_dumped;
    i.loading_dumped |= dumped_like || file.starts_with("builtin:");
    let r = eval_str_for_load(i, &src);
    i.loading_dumped = was_dumped;
    i.lexenv = saved_lexenv;
    if r.is_ok() {
        // GNU records the file in `load-history': (FILE . ENTRIES),
        // newest file first, entries in evaluation order.
        let mut entries = i.symbol_value(cll).list_to_vec().unwrap_or_default();
        entries.reverse();
        let entry = Value::cons(Value::string(file), Value::list(entries));
        let lh = i.intern("load-history");
        let cur = match i.symbol_value(lh) {
            // Loads can run before the prelude `defvar' initializes it.
            Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
            v => v,
        };
        i.obarray.symbol_mut(lh).value = Value::cons(entry, cur);
        run_after_load(i, file);
    }
    i.unbind_to(mark)?;
    r
}

/// Fire `after-load-alist' entries whose regexp/symbol key matches FILE
/// (GNU's `load' runs `eval-after-load' hooks this way).
fn run_after_load(i: &mut Interp, file: &str) {
    let alist_sym = i.intern("after-load-alist");
    let sm_sym = i.intern("string-match");
    let entries = i.symbol_value(alist_sym).list_to_vec().unwrap_or_default();
    for entry in entries {
        let mut parts = entry.list_to_vec().unwrap_or_default();
        if parts.is_empty() {
            continue;
        }
        let key = parts.remove(0);
        let matches = match key {
            Value::Str(_) => i
                .call_function(
                    &Value::Sym(sm_sym),
                    &Value::list(vec![key.clone(), Value::string(file)]),
                    Some(sm_sym),
                )
                .map(|v| !v.is_nil())
                .unwrap_or(false),
            Value::Sym(s) => {
                // Feature-name keys match the file's basename feature.
                file.ends_with(&format!("{}.el", i.symbol_name(s)))
            }
            _ => false,
        };
        if matches {
            for form in parts {
                // Entries store 0-arg functions (lambdas or closures).
                let _ = i.apply(&form, vec![]);
            }
        }
    }
}

/// Read and eval each top-level form, eagerly expanding macros the way
/// GNU's `internal-macroexpand-for-load' does during `load'.  This is
/// what resolves macro autoloads (e.g. `define-minor-mode') at load
/// time rather than first call.
fn eval_str_for_load(i: &mut Interp, src: &str) -> EvalResult {
    let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
    let mut pos = 0usize;
    let mut last = Value::Nil;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(pos);
            match reader.read()? {
                Some(f) => Some((f, reader.position())),
                None => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                match eval_for_load(i, form.clone()) {
                    Ok(v) => last = v,
                    Err(f) => {
                        if std::env::var_os("REMACS_TRACE_ERR").is_some() {
                            let fdesc = match &f {
                                crate::lisp::Flow::Signal(s, d, _) => format!(
                                    "signal {} {}",
                                    i.prin1_to_string(s),
                                    i.prin1_to_string(d).chars().take(120).collect::<String>()
                                ),
                                other => format!("{:?}", other),
                            };
                            eprintln!(
                                "[load-err@{}] {} => {}",
                                end,
                                i.princ_to_string(&form)
                                    .chars()
                                    .take(120)
                                    .collect::<String>(),
                                fdesc
                            );
                        }
                        return Err(f);
                    }
                }
            }
            None => return Ok(last),
        }
    }
}

/// Expand + eval one top-level form the way GNU's
/// `internal-macroexpand-for-load' does during `load': the form is
/// expanded; a top-level `progn' is spliced so each child is expanded
/// and evaluated in turn (an autoloaded macro in a later child is not
/// resolved until that child's expansion runs, matching GNU's
/// observable autoload timing).
fn eval_for_load(i: &mut Interp, form: Value) -> EvalResult {
    // Splice a `progn' BEFORE expanding: GNU expands each spliced child
    // at its own turn, so later autoloaded macros stay unresolved while
    // earlier children evaluate.
    if let Some(children) = progn_children(i, &form) {
        let mut last = Value::Nil;
        for child in children {
            last = eval_for_load(i, child)?;
        }
        return Ok(last);
    }
    i.macroexp_call_depth += 1;
    let expanded_result = crate::lisp::builtins::evalfn::macroexpand_all(i, &form);
    i.macroexp_call_depth -= 1;
    let expanded = match expanded_result {
        Ok(f) => f,
        // GNU's `internal-macroexpand-for-load' wraps expansion
        // failures: it re-signals (error "Eager macro-expansion
        // failure: %S" err) where err is the original condition as
        // (sym . data); `error' formats eagerly, so the data is a
        // single formatted string.  Non-signal exits propagate.
        Err(crate::lisp::Flow::Signal(sym, data, _)) => {
            let err = Value::cons(sym, data);
            let msg = crate::lisp::builtins::evalfn::apply_format_simple(
                i,
                "Eager macro-expansion failure: %S",
                &[err],
            );
            return Err(i.error(msg));
        }
        Err(f) => return Err(f),
    };
    // A macro may have expanded into a top-level `progn' — splice it too.
    if let Some(children) = progn_children(i, &expanded) {
        let mut last = Value::Nil;
        for child in children {
            last = eval_for_load(i, child)?;
        }
        return Ok(last);
    }
    match i.eval(&expanded) {
        Ok(v) => {
            record_load_entry(i, &expanded);
            Ok(v)
        }
        Err(crate::lisp::Flow::Throw(tag, val)) => {
            let nc = i.intern("no-catch");
            Err(i.signal_data(nc, vec![tag, val]))
        }
        Err(f) => Err(f),
    }
}

/// Record a `load-history' entry for the embedded startup prelude so
/// `symbol-file' and the find-func family can locate prelude-defined
/// symbols.  GNU records every dumped library in `load-history' the same
/// way; FILE names the on-disk copy of the prelude source.
pub(crate) fn record_prelude_load_history(i: &mut Interp, file: &str, src: &str) {
    let cll = i.intern("current-load-list");
    let mark = i.specbind_depth();
    if i.specbind(cll, Value::Nil).is_err() {
        return;
    }
    let chars: Rc<Vec<char>> = Rc::new(src.chars().collect());
    let mut pos = 0usize;
    loop {
        let next = {
            let mut reader = crate::lisp::reader::Reader::with_chars(i, chars.clone());
            reader.set_position(pos);
            match reader.read() {
                Ok(f) => f.map(|v| (v, reader.position())),
                Err(_) => None,
            }
        };
        match next {
            Some((form, end)) => {
                pos = end;
                record_form_tree(i, &form);
            }
            None => break,
        }
    }
    let mut entries = i.symbol_value(cll).list_to_vec().unwrap_or_default();
    entries.reverse();
    let entry = Value::cons(Value::string(file), Value::list(entries));
    let lh = i.intern("load-history");
    let cur = match i.symbol_value(lh) {
        Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
        v => v,
    };
    i.obarray.symbol_mut(lh).value = Value::cons(entry, cur);
    let _ = i.unbind_to(mark);
}

/// Record one top-level prelude form, splicing `progn' children the way
/// `eval_for_load' does during `load'.
fn record_form_tree(i: &mut Interp, form: &Value) {
    if let Some(children) = progn_children(i, form) {
        for child in children {
            record_form_tree(i, &child);
        }
    } else {
        record_load_entry(i, form);
    }
}

/// Record a `load-history' entry for a successfully evaluated top-level
/// form during `load', pushing onto `current-load-list' the way GNU's
/// lread.c does.  GNU's entry shapes: `(defun . SYM)' for function-ish
/// definitions (defun/defmacro/defsubst/defalias/autoload, plus the
/// `defun' leaf that mode/defgeneric macros expand into), bare `SYM'
/// for variables, `(defface . SYM)', `(provide . FEAT)' and
/// `(require . FEAT)'.
fn record_load_entry(i: &mut Interp, form: &Value) {
    let Some(items) = form.list_to_vec().ok() else {
        return;
    };
    let Some(&Value::Sym(head)) = items.first() else {
        return;
    };
    // NAME is (cadr FORM); unwrap a `quote' wrapper (`provide' et al).
    let quote_id = i.intern("quote");
    let name = |items: &[Value]| -> Option<Value> {
        match items.get(1)? {
            v @ Value::Sym(_) => Some(v.clone()),
            Value::Cons(_) => {
                let q = items[1].list_to_vec().ok()?;
                if q.len() == 2 && matches!(&q[0], Value::Sym(s) if *s == quote_id) {
                    Some(q[1].clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    };
    const DEFUN_HEADS: &[&str] = &[
        "defun",
        "defmacro",
        "defsubst",
        "defalias",
        "autoload",
        "cl-defun",
        "cl-defmacro",
        "define-minor-mode",
        "define-derived-mode",
        "cl-defgeneric",
        "cl-defmethod",
        "define-obsolete-function-alias",
    ];
    const DEFVAR_HEADS: &[&str] = &[
        "defvar",
        "defconst",
        "defvar-local",
        "defcustom",
        "defvar-keymap",
        "custom-declare-variable",
        "define-obsolete-variable-alias",
    ];
    let head_name = i.symbol_name(head).to_string();
    let defun_id = i.intern("defun");
    let defface_id = i.intern("defface");
    let provide_id = i.intern("provide");
    let require_id = i.intern("require");
    let entry = match head_name.as_str() {
        h if DEFUN_HEADS.contains(&h) => name(&items).map(|nm| Value::cons(i.sym(defun_id), nm)),
        h if DEFVAR_HEADS.contains(&h) => name(&items),
        // `defface' expands to `custom-declare-face' before eval; GNU's
        // C subr attaches (defface . FACE) to `current-load-list'.
        "defface" | "custom-declare-face" => {
            name(&items).map(|nm| Value::cons(i.sym(defface_id), nm))
        }
        "provide" => name(&items).map(|nm| Value::cons(i.sym(provide_id), nm)),
        "require" => name(&items).map(|nm| Value::cons(i.sym(require_id), nm)),
        _ => None,
    };
    if let Some(e) = entry {
        let cll = i.intern("current-load-list");
        let cur = match i.symbol_value(cll) {
            Value::Sym(s) if s == crate::lisp::sym::UNBOUND => Value::Nil,
            v => v,
        };
        i.obarray.symbol_mut(cll).value = Value::cons(e, cur);
    }
}

/// If FORM is `(progn CHILDREN...)', return the children.
fn progn_children(i: &mut Interp, form: &Value) -> Option<Vec<Value>> {
    if let Value::Cons(c) = form {
        let cb = c.borrow();
        if let Value::Sym(s) = &cb.car {
            if *s == i.intern("progn") {
                return cb.cdr.list_to_vec().ok();
            }
        }
    }
    None
}

/// True when the file declares `-*- lexical-binding: t -*-' (or the
/// `lexical-binding: t' local-variable form) on its first line — or on
/// the second line when the first is a `#!' line, like Emacs.
fn file_lexical_binding(src: &str) -> bool {
    let mut lines = src.lines();
    let first = lines.next().unwrap_or("");
    let probe = if first.starts_with("#!") {
        lines.next().unwrap_or("")
    } else {
        first
    };
    probe.contains("lexical-binding: t") || probe.contains("lexical-binding:t")
}

/// Load library NAME; returns true if a file was found and loaded.
pub(crate) fn load_library(i: &mut Interp, name: &str) -> Result<bool, crate::lisp::Flow> {
    load_library_opts(i, name, false, false, true)
}

/// `load_library' honoring `load''s NOSUFFIX, MUST-SUFFIX, and
/// NOMESSAGE arguments.  Like GNU, `Loading %s...' is echoed (through
/// `message') before the file's forms run; ` (source)' marks files
/// read from Lisp source rather than byte-compiled output.
pub(crate) fn load_library_opts(
    i: &mut Interp,
    name: &str,
    nosuffix: bool,
    mustsuffix: bool,
    nomessage: bool,
) -> Result<bool, crate::lisp::Flow> {
    let announce = |i: &mut Interp, file: &str| {
        if nomessage {
            return;
        }
        let fmt = if file.ends_with(".el") {
            "Loading %s (source)..."
        } else {
            "Loading %s..."
        };
        let msg = i.intern("message");
        let _ = i.apply(
            &Value::Sym(msg),
            vec![Value::string(fmt), Value::string(file)],
        );
    };
    // A bundled library stands in for GNU's shipped .elc: its
    // macroexpansion ran at byte-compile time, so the `gensym-counter'
    // bumps our interpreted load performs must not leak into the
    // session.  Save/restore it around the load.
    let stem = name
        .strip_suffix(".el")
        .or_else(|| name.strip_suffix(".elc"))
        .unwrap_or(name);
    let bundled = embedded(stem).is_some();
    let gc = i.intern("gensym-counter");
    let saved_gc = bundled.then(|| i.symbol_value(gc));
    let result = match locate_opts(i, name, nosuffix, mustsuffix) {
        Some(path) => {
            announce(i, &path);
            eval_file_lex_dumped(i, &path, false, bundled).map(|_| true)
        }
        // Fall back to the embedded copy of a built-in library, so that
        // autoloads work even when the lisp/ dir isn't reachable by path.
        None => match embedded(name) {
            Some(src) => {
                announce(i, name);
                let virtual_path = format!("builtin:{}", name);
                eval_src(i, &virtual_path, src, false).map(|_| true)
            }
            None => Ok(false),
        },
    };
    if let Some(v) = saved_gc {
        i.obarray.symbol_mut(gc).value = v;
    }
    result
}
