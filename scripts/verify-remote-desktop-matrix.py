#!/usr/bin/env python3
"""Validate the canonical remote desktop acceptance matrix CSV (F17)."""

import csv
import re
import sys
from pathlib import Path

REQUIRED_HEADER = [
    "source_sha",
    "artifact_sha256",
    "matrix_id",
    "test_id",
    "subcase",
    "layer",
    "client_os",
    "architecture",
    "display_backend",
    "scale",
    "server_version",
    "negotiated_capabilities",
    "expected",
    "actual",
    "status",
    "evidence_ref",
    "blocker",
]

VALID_STATUSES = {"PASS", "FAIL", "BLOCKED", "NOT RUN", "N/A"}
VALID_LAYERS = {"L1", "L2", "L3", "L4", "L5"}

EXPECTED_TEST_IDS = (
    [f"I{i:02d}" for i in range(1, 21)]
    + [f"C{i:02d}" for i in range(1, 25)]
    + [f"D{i:02d}" for i in range(1, 20)]
    + [f"A{i:02d}" for i in range(1, 10)]
)
EXPECTED_MATRIX_IDS = [f"M{i:02d}" for i in range(1, 16)]

SHA_RE = re.compile(r"^[0-9a-f]{40}$")
SINGLE_TEST_ID_RE = re.compile(r"^[ICDA]\d{2}$")


def validate_rows(header: list[str], rows: list[dict[str, str]]) -> list[str]:
    errors: list[str] = []
    if header != REQUIRED_HEADER:
        errors.append(f"Invalid header: {header} != {REQUIRED_HEADER}")
        return errors

    seen_keys: set[tuple[str, str, str, str]] = set()
    seen_test_ids: set[str] = set()
    seen_matrix_ids: set[str] = set()

    for idx, row in enumerate(rows, start=2):
        sha = row["source_sha"].strip()
        if not SHA_RE.match(sha):
            errors.append(f"Row {idx}: invalid or placeholder source_sha '{sha}'")

        layer = row["layer"].strip()
        if layer not in VALID_LAYERS:
            errors.append(f"Row {idx}: invalid or combined layer '{layer}'")

        test_id = row["test_id"].strip()
        if not SINGLE_TEST_ID_RE.match(test_id):
            errors.append(f"Row {idx}: invalid or ranged test_id '{test_id}'")

        matrix_id = row["matrix_id"].strip()
        if matrix_id not in EXPECTED_MATRIX_IDS:
            errors.append(f"Row {idx}: invalid matrix_id '{matrix_id}'")

        status = row["status"].strip()
        if status not in VALID_STATUSES:
            errors.append(f"Row {idx}: invalid status '{status}'")

        if status == "PASS" and not row["evidence_ref"].strip():
            errors.append(f"Row {idx}: PASS requires non-empty evidence_ref")
        if status == "BLOCKED" and not row["blocker"].strip():
            errors.append(f"Row {idx}: BLOCKED requires non-empty blocker")
        if status == "N/A" and not row["actual"].strip():
            errors.append(f"Row {idx}: N/A requires justification in actual")

        key = (matrix_id, test_id, row["subcase"].strip(), layer)
        if key in seen_keys:
            errors.append(f"Row {idx}: duplicate key {key}")
        seen_keys.add(key)
        seen_test_ids.add(test_id)
        seen_matrix_ids.add(matrix_id)

    missing_tests = sorted(set(EXPECTED_TEST_IDS) - seen_test_ids)
    if missing_tests:
        errors.append(f"Missing catalog test IDs: {missing_tests}")

    missing_matrices = sorted(set(EXPECTED_MATRIX_IDS) - seen_matrix_ids)
    if missing_matrices:
        errors.append(f"Missing matrix IDs: {missing_matrices}")

    return errors


def validate_csv(path: Path) -> list[str]:
    with path.open(newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        header = reader.fieldnames or []
        rows = list(reader)
    return validate_rows(header, rows)


def run_self_tests() -> None:
    # Negative test: HEAD placeholder, ranged test_id, combined layer, missing PASS evidence
    bad_rows = [
        {
            "source_sha": "HEAD",
            "artifact_sha256": "",
            "matrix_id": "M01",
            "test_id": "I01-I20",
            "subcase": "all",
            "layer": "L4/L5",
            "client_os": "macOS",
            "architecture": "arm64",
            "display_backend": "Cocoa",
            "scale": "2x",
            "server_version": "Win11",
            "negotiated_capabilities": "RDP",
            "expected": "ok",
            "actual": "ok",
            "status": "PASS",
            "evidence_ref": "",
            "blocker": "",
        }
    ]
    errs = validate_rows(REQUIRED_HEADER, bad_rows)
    assert any("source_sha" in e for e in errs)
    assert any("layer" in e for e in errs)
    assert any("test_id" in e for e in errs)
    assert any("evidence_ref" in e for e in errs)
    assert any("Missing catalog test IDs" in e for e in errs)


def main() -> int:
    run_self_tests()
    csv_path = Path("docs/qa/remote-desktop-matrix.csv")
    if len(sys.argv) > 1:
        csv_path = Path(sys.argv[1])
    errors = validate_csv(csv_path)
    if errors:
        for e in errors[:25]:
            print(f"ERROR: {e}", file=sys.stderr)
        print(f"Total validation errors: {len(errors)}", file=sys.stderr)
        return 1
    with csv_path.open(newline="", encoding="utf-8") as fh:
        rows = list(csv.DictReader(fh))
    counts: dict[str, int] = {}
    for r in rows:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    print(f"Matrix validation PASSED ({len(rows)} rows): {counts}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
