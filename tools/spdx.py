# SPDX-License-Identifier: MIT
"""Add or check the SPDX licence line on commy's source files.

    python tools/spdx.py --check     # exit 1 if any file lacks the line (CI mode)
    python tools/spdx.py             # add the line where it is missing

Files are the ones git tracks (`git ls-files`), never a disk walk, so another worktree is never touched.
The header goes on line 1, or line 2 under a shebang. A file with a different SPDX identifier is
reported and left alone (exit 3); an empty file is skipped. Line endings and a UTF-8 BOM are kept.
"""

import argparse
import pathlib
import subprocess
import sys

LICENSE = "MIT"
MARKER = "SPDX-License-Identifier:"
PREFIX = {".rs": "// ", ".py": "# ", ".sh": "# ", ".ps1": "# "}
WINDOW = 10
BOM = "﻿"


def tracked(root):
    out = subprocess.run(["git", "-C", str(root), "ls-files", "-z"], capture_output=True, check=True).stdout
    return [root / n for n in out.decode("utf-8").split("\0") if n]


def declared(text):
    for line in text.splitlines()[:WINDOW]:
        if MARKER in line:
            return line.split(MARKER, 1)[1].strip()
    return None


def stamped(text, ext):
    bom = text.startswith(BOM)
    body = text[len(BOM):] if bom else text
    nl = "\r\n" if "\r\n" in body else "\n"
    lines = body.split(nl)
    at = 1 if lines[0].startswith("#!") else 0
    lines.insert(at, f"{PREFIX[ext]}{MARKER} {LICENSE}")
    return (BOM if bom else "") + nl.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="report only; exit 1 if a file lacks the line")
    parser.add_argument("--root", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent.parent)
    args = parser.parse_args()

    files = [p for p in tracked(args.root) if p.suffix in PREFIX and p.is_file()]
    missing, other, empty, done = [], [], 0, 0
    for path in files:
        text = path.read_bytes().decode("utf-8")
        if not text.strip():
            empty += 1
            continue
        found = declared(text)
        if found == LICENSE:
            done += 1
        elif found is not None:
            other.append((path, found))
        else:
            missing.append(path)
            if not args.check:
                path.write_bytes(stamped(text, path.suffix).encode("utf-8"))
    print(f"enumerated by git ls-files: {len(files)} source files ({', '.join(sorted(PREFIX))}); "
          f"{done} already carry {LICENSE}, {len(missing)} {'missing' if args.check else 'stamped'}, {empty} empty")
    for path, found in other:
        print(f"  CONFLICT {path.relative_to(args.root)}: declares {found!r}, not {LICENSE!r}")
    if args.check:
        for path in missing:
            print(f"  MISSING {path.relative_to(args.root)}")
    if other:
        return 3
    return 1 if (args.check and missing) else 0


if __name__ == "__main__":
    sys.exit(main())
