#!/usr/bin/env python3
"""Generate EMBEDDED_LISP entries for src/lisp/load.rs.

Scans lisp/*.el, matches each file against the GNU Emacs source tree by
content hash, and emits (key, include_str!(path)) lines.  The key is the
GNU load path (e.g. "semantic/ctxt" for a file originally at
cedet/semantic/ctxt.el) so that qualified `require' calls resolve via
embedded()'s exact-match lookup; flat keys are used otherwise.

Usage:
    tools/gen_embedded.py [--gnu DIR] [--lisp DIR]

Files that cannot be matched to a GNU source are emitted with their
basename and a comment so they can be reviewed by hand.
"""

import argparse
import gzip
import hashlib
import os
import sys
from pathlib import Path

DEFAULT_GNU = (
    "/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1"
    "/share/emacs/31.1/lisp"
)

# lisp/ basenames that are intentionally not GNU-verbatim (adapted ports,
# remacs-specific files).  They still get a flat key; the GNU path is only
# used to pick qualified keys for renamed/shadowed copies.
QUALIFY_PREFIXES = ("semantic-", "ede-", "srecode-", "quail-")


def gnu_map(gnu_dir: Path) -> dict:
    """md5 hex -> relative load path (no extension) for every GNU .el."""
    out = {}
    for root, _dirs, files in os.walk(gnu_dir):
        for name in files:
            p = Path(root) / name
            if name.endswith(".el.gz"):
                data = gzip.decompress(p.read_bytes())
                stem = name[:-6]
            elif name.endswith(".el"):
                data = p.read_bytes()
                stem = name[:-3]
            else:
                continue
            rel = p.relative_to(gnu_dir)
            key = str(rel.parent / stem) if str(rel.parent) != "." else stem
            # GNU puts every first-level lisp/ subdir on load-path, so the
            # load key drops that component: cedet/semantic/ctxt -> semantic/ctxt,
            # leim/quail/emoji -> quail/emoji, calendar/icalendar -> icalendar.
            parts = key.split("/")
            if len(parts) > 1:
                key = "/".join(parts[1:])
            out.setdefault(hashlib.md5(data).hexdigest(), key)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gnu", default=DEFAULT_GNU, help="GNU lisp source dir")
    ap.add_argument("--lisp", default="lisp", help="repo lisp dir")
    ap.add_argument(
        "--prefix",
        default="../../lisp/",
        help="include_str! path prefix from load.rs",
    )
    args = ap.parse_args()

    hashes = gnu_map(Path(args.gnu))
    lines = []
    for f in sorted(Path(args.lisp).glob("*.el")):
        raw = f.read_bytes()
        try:
            raw.decode("utf-8")
        except UnicodeDecodeError:
            # include_str! requires UTF-8; these files carry raw
            # non-UTF-8 bytes (Emacs-internal encodings, Big5 tables).
            # Leave them to the filesystem fallback in builtin_dirs().
            lines.append(f"    // {f.name} skipped: not valid UTF-8")
            continue
        digest = hashlib.md5(raw).hexdigest()
        key = hashes.get(digest, f.stem)
        # A GNU-qualified key only makes sense when the file was renamed
        # (basename differs); identical basenames keep the flat key so
        # `require' on the flat name also resolves.
        if "/" in key and os.path.basename(key) == f.stem:
            key = f.stem
        lines.append(f'    ("{key}", include_str!("{args.prefix}{f.name}")),')
        if key != f.stem and "/" in key:
            lines[-1] += f"  // GNU {key}"
        elif digest not in hashes:
            lines[-1] += "  // not in GNU tree (adapted/remacs-specific)"
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
