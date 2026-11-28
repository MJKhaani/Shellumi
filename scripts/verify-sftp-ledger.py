#!/usr/bin/env python3
"""Validate the 109-row SFTP acceptance ledger in docs/development/sftp-implementation-progress.md."""
import re
import sys
from collections import Counter
from pathlib import Path

LEDGER = Path("docs/development/sftp-implementation-progress.md")
VALID_STATUSES = {
    "PASS",
    "IMPLEMENTED / NOT TESTED",
    "NOT TESTED",
    "FAIL",
    "NOT IMPLEMENTED",
}

EXPECTED_IDS = (
    [f"A{i:02d}" for i in range(1, 15)]
    + [f"B{i:02d}" for i in range(1, 13)]
    + [f"C{i:02d}" for i in range(1, 21)]
    + [f"D{i:02d}" for i in range(1, 17)]
    + [f"E{i:02d}" for i in range(1, 18)]
    + [f"F{i:02d}" for i in range(1, 19)]
    + [f"G{i:02d}" for i in range(1, 13)]
)

text = LEDGER.read_text(encoding="utf-8")
rows = {}
for line in text.splitlines():
    m = re.match(r"^\|\s*([A-G]\d\d)\s*\|", line)
    if not m:
        continue
    cid = m.group(1)
    if cid in rows:
        print(f"ERROR: Duplicate ID {cid}", file=sys.stderr)
        sys.exit(1)
    parts = [p.strip() for p in line.split("|")[1:-1]]
    if len(parts) < 7:
        print(f"ERROR: Malformed row for {cid}: {line}", file=sys.stderr)
        sys.exit(1)
    status = parts[1]
    if status not in VALID_STATUSES:
        print(f"ERROR: Invalid status '{status}' for {cid}", file=sys.stderr)
        sys.exit(1)
    rows[cid] = status

missing = [cid for cid in EXPECTED_IDS if cid not in rows]
extra = [cid for cid in rows if cid not in EXPECTED_IDS]
if missing or extra or len(rows) != 109:
    print(f"ERROR: Expected 109 IDs; got {len(rows)}, missing={missing}, extra={extra}", file=sys.stderr)
    sys.exit(1)

counts = Counter(rows.values())
print(
    f"SFTP ledger OK: 109 rows | PASS={counts['PASS']} | "
    f"IMPLEMENTED / NOT TESTED={counts['IMPLEMENTED / NOT TESTED']} | "
    f"NOT TESTED={counts['NOT TESTED']} | FAIL={counts['FAIL']} | "
    f"NOT IMPLEMENTED={counts['NOT IMPLEMENTED']}"
)
