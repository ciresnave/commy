import re
from pathlib import Path

log = Path("gh-logs/run-17799343414.log")
if not log.exists():
    print("Log file not found:", log)
    raise SystemExit(1)
text = log.read_text(encoding="utf-8", errors="replace")
lines = text.splitlines()
patterns = [
    ("capnp_success", re.compile(r"capnp codegen succeeded", re.IGNORECASE)),
    ("copied_schema", re.compile(r"copied schema:", re.IGNORECASE)),
    ("schema_preview", re.compile(r"schema-preview:", re.IGNORECASE)),
    ("found_generated", re.compile(r"found generated file", re.IGNORECASE)),
    ("moved_generated", re.compile(r"moved generated", re.IGNORECASE)),
    ("couldnt_read", re.compile(r"couldn't read", re.IGNORECASE)),
    (
        "include_example",
        re.compile(
            r'include!\(concat!\(env!\("OUT_DIR"\), "/example_capnp.rs"\)\)',
            re.IGNORECASE,
        ),
    ),
]

matches = []
for i, line in enumerate(lines, start=1):
    for name, pat in patterns:
        if pat.search(line):
            # collect context
            start = max(0, i - 4)
            end = min(len(lines), i + 3)
            ctxt = lines[start:end]
            matches.append((i, name, line, ctxt))

if not matches:
    print("No matches for patterns found in log")
else:
    for i, name, line, ctxt in matches:
        print(f"--- Match ({name}) at line {i} ---")
        for j, l in enumerate(ctxt, start=max(1, i - 3)):
            prefix = ">>" if j == i else "  "
            print(f"{prefix} {j}: {l}")
        print()

# Also print a short summary of capnpc locations
print("\n--- capnpc location search ---")
for i, line in enumerate(lines, start=1):
    if "capnpc:" in line:
        print(f"{i}: {line}")
    if re.search(r"command -v capnpc", line):
        print(f"{i}: {line}")

print("\nParsing complete")
