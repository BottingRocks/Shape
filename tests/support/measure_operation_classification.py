#!/usr/bin/env python3
"""Reproducible before/after measurement for operation-handler classification.

This harness is measurement-only. It invokes the already-built release `shape`
binary (`shape ops list --format json`), which deobfuscates a source and then
catalogues the proven operation table. It never executes seed JavaScript/WASM
and it never mutates the catalogue. All "proposed" values are recomputed in
the harness from the current record's recognized micro-operation count and
unknown-fragment count; no production code is changed.

Usage:
    python3 tests/support/measure_operation_classification.py \
        --shape target/release/shape \
        --seeds /path/to/seeds \
        --out   /tmp/shape-operations-classification-findings

Outputs (JSON):
    manifest.json       seed inventory, sizes, sha256 digests
    baseline.json       per-source current distribution and cross-tabs
    proposed.json       per-source proposed distribution and field diffs
    summary.json        compact before/after summary and STOP decision
    raw/<source>.ops-list.json   exact `ops list` payload for each source
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from collections import Counter, OrderedDict

# Current classifier (src/operations/catalogue.rs, classify_handler):
#   recognized == 0 || (unknown and recognized == 1) -> unknown
#   no unknown and recognized == 1                    -> atomic
#   otherwise                                         -> composite
CURRENT_PROOF = {
    "unknown": "OP-UNKNOWN-001",
    "atomic": "OP-AST-ATOMIC-001",
    "composite": "OP-AST-COMPOSITE-001",
}
CURRENT_EVIDENCE = {
    "unknown": "No bounded recognizer matched; body retained verbatim",
    "atomic": "One exact AST recognizer matched and no unknown fragments were observed",
    "composite": "Ordered exact recognizers retained; unsupported fragments remain explicit",
}

# Alternate monotone rule for comparison. Classification strings are a closed
# set; this script measures the impact of adding `partial` without changing the
# current catalogue contract.
PROPOSED_LEVEL = {
    "unknown": "unknown",
    "partial": "partial",
    "atomic": "high",
    "composite": "high",
}
PROPOSED_PROOF = {
    "unknown": "OP-UNKNOWN-001",
    "partial": "OP-AST-PARTIAL-001",
    "atomic": "OP-AST-ATOMIC-001",
    "composite": "OP-AST-COMPOSITE-002",
}
PROPOSED_EVIDENCE = {
    "unknown": "No bounded recognizer matched; body retained verbatim",
    "partial": "At least one exact AST recognizer matched; unsupported fragments remain explicit",
    "atomic": "One exact AST recognizer matched and no unknown fragments were observed",
    "composite": "Two or more exact AST recognizers matched and no unknown fragments were observed",
}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def recognized_bucket(count):
    if count == 0:
        return "0"
    if count == 1:
        return "1"
    return "2+"


def proposed_classification(recognized, unknown_count):
    if recognized == 0:
        return "unknown"
    if unknown_count > 0:
        return "partial"
    if recognized == 1:
        return "atomic"
    return "composite"


def proposed_inventory(rows):
    inventory = OrderedDict([("exact", 0), ("partial", 0), ("unknown", 0)])
    for row in rows:
        classification = proposed_classification(
            row["recognized_count"], row["unknown_count"]
        )
        if classification in ("atomic", "composite"):
            inventory["exact"] += 1
        elif classification == "partial":
            inventory["partial"] += 1
        else:
            inventory["unknown"] += 1
    return inventory


def cross_tab(rows, key_field):
    """Return {(recognized_bucket, has_unknown): {value: count}}."""
    table = OrderedDict()
    for row in rows:
        bucket = row["recognized_bucket"]
        has_unknown = "yes" if row["unknown_count"] > 0 else "no"
        value = row[key_field]
        table.setdefault((bucket, has_unknown), Counter())[value] += 1
    return table


def run_ops_list(shape, path, timeout=1800):
    command = [shape, "ops", "list", path, "--format", "json"]
    try:
        completed = subprocess.run(
            command, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return {"status": "timeout", "command": command, "error": "timed out"}
    if completed.returncode != 0:
        return {
            "status": "error",
            "command": command,
            "exit_code": completed.returncode,
            "error": completed.stderr.strip() or completed.stdout.strip(),
        }
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        return {
            "status": "error",
            "command": command,
            "exit_code": completed.returncode,
            "error": "invalid JSON: %s" % error,
        }
    return {
        "status": "ok",
        "command": command,
        "exit_code": completed.returncode,
        "payload": payload,
    }


def analyze_handlers(payload):
    rows = []
    table = payload.get("table", {})
    raw_handlers = payload.get("handlers", [])
    for handler in raw_handlers:
        recognized = len(handler.get("micro_ops", []))
        unknown_count = handler.get("unknown_fragments", 0)
        family = handler.get("family", "")
        current_class = handler.get("classification", "")
        current_level = handler.get("confidence", "")
        proposed_class = proposed_classification(recognized, unknown_count)
        rows.append(
            {
                "key": handler.get("key"),
                "index": handler.get("index"),
                "family": family,
                "recognized_count": recognized,
                "recognized_bucket": recognized_bucket(recognized),
                "unknown_count": unknown_count,
                "current_classification": current_class,
                "current_confidence_level": current_level,
                "current_proof_rule_id": CURRENT_PROOF.get(current_class, "?"),
                "current_evidence": CURRENT_EVIDENCE.get(current_class, "?"),
                "proposed_classification": proposed_class,
                "proposed_confidence_level": PROPOSED_LEVEL[proposed_class],
                "proposed_proof_rule_id": PROPOSED_PROOF[proposed_class],
                "proposed_evidence": PROPOSED_EVIDENCE[proposed_class],
                # Semantic family is deliberately unchanged by the proposal.
                "proposed_semantic_family": family,
            }
        )
    return rows, table


def preserved_projection_digest(payload):
    """Digest of the `ops list` fields the proposal must not touch.

    Spans, canonical normalized bodies and effects are only exposed by
    `ops show`; the full-record preservation check belongs to the deferred
    implementation. This digest pins every non-classification field the list
    view exposes (identity, family, fingerprints, micro-operations and the
    unknown-fragment count) so a future implementation can show equality.
    """
    projection = OrderedDict()
    projection["derived_from"] = payload.get("derived_from")
    projection["capture"] = payload.get("capture")
    projection["table"] = payload.get("table")
    projection["handlers"] = [
        OrderedDict(
            (field, handler.get(field))
            for field in ("key", "index", "family", "micro_ops", "fingerprint", "unknown_fragments")
        )
        for handler in payload.get("handlers", [])
    ]
    encoded = json.dumps(projection, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode()).hexdigest()


def summarize_source(name, kind, path, run):
    result = OrderedDict(
        [
            ("source", name),
            ("source_kind", kind),
            ("path", path),
            ("status", run["status"]),
        ]
    )
    if run["status"] != "ok":
        result["error"] = run.get("error")
        result["exit_code"] = run.get("exit_code")
        return result, None

    payload = run["payload"]
    rows, table = analyze_handlers(payload)
    current_inventory = payload.get("inventory", {})
    current_class = Counter(row["current_classification"] for row in rows)
    current_level = Counter(row["current_confidence_level"] for row in rows)
    current_proof = Counter(row["current_proof_rule_id"] for row in rows)
    proposed_class = Counter(row["proposed_classification"] for row in rows)
    proposed_level = Counter(row["proposed_confidence_level"] for row in rows)
    proposed_proof = Counter(row["proposed_proof_rule_id"] for row in rows)

    changed_class = [r for r in rows if r["current_classification"] != r["proposed_classification"]]
    changed_level = [
        r for r in rows if r["current_confidence_level"] != r["proposed_confidence_level"]
    ]
    changed_proof = [
        r for r in rows if r["current_proof_rule_id"] != r["proposed_proof_rule_id"]
    ]
    changed_evidence = [r for r in rows if r["current_evidence"] != r["proposed_evidence"]]
    changed_family = [
        r for r in rows if r["family"] != r["proposed_semantic_family"]
    ]

    total = len(rows)
    result.update(
        OrderedDict(
            [
                (
                    "table",
                    {
                        "declaration_ordinal": table.get("declaration_ordinal"),
                        "canonical_name": table.get("canonical_name"),
                        "shape": table.get("shape"),
                        "element_count": table.get("element_count"),
                        "function_count": table.get("function_count"),
                    },
                ),
                ("handler_count", total),
                (
                    "preserved_projection_sha256",
                    preserved_projection_digest(payload),
                ),
                ("current_inventory", current_inventory),
                ("current_classification", dict(current_class)),
                ("current_confidence_level", dict(current_level)),
                ("current_proof_rule_id", dict(current_proof)),
                ("proposed_inventory", proposed_inventory(rows)),
                ("proposed_classification", dict(proposed_class)),
                ("proposed_confidence_level", dict(proposed_level)),
                ("proposed_proof_rule_id", dict(proposed_proof)),
                (
                    "baseline_cross_tab",
                    jsonable_cross_tab(cross_tab(rows, "current_classification")),
                ),
                (
                    "proposed_cross_tab",
                    jsonable_cross_tab(cross_tab(rows, "proposed_classification")),
                ),
                (
                    "changed",
                    OrderedDict(
                        [
                            ("classification", len(changed_class)),
                            ("confidence_level", len(changed_level)),
                            ("proof_rule_id", len(changed_proof)),
                            ("evidence", len(changed_evidence)),
                            ("semantic_family", len(changed_family)),
                        ]
                    ),
                ),
                (
                    "changed_percent",
                    OrderedDict(
                        [
                            ("classification", round(100.0 * len(changed_class) / total, 3) if total else 0.0),
                            ("confidence_level", round(100.0 * len(changed_level) / total, 3) if total else 0.0),
                            ("proof_rule_id", round(100.0 * len(changed_proof) / total, 3) if total else 0.0),
                            ("evidence", round(100.0 * len(changed_evidence) / total, 3) if total else 0.0),
                        ]
                    ),
                ),
                (
                    "changed_examples",
                    [
                        {
                            "key": r["key"],
                            "index": r["index"],
                            "recognized_count": r["recognized_count"],
                            "unknown_count": r["unknown_count"],
                            "current": r["current_classification"],
                            "proposed": r["proposed_classification"],
                        }
                        for r in changed_class[:10]
                    ],
                ),
            ]
        )
    )
    return result, rows


def jsonable_cross_tab(table):
    out = OrderedDict()
    for (bucket, unknown), counts in table.items():
        out["%s|unknown=%s" % (bucket, unknown)] = dict(counts)
    return out


def fmt_counter(counter):
    if not counter:
        return "-"
    return ", ".join("%s=%d" % (key, counter[key]) for key in sorted(counter))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--shape", required=True, help="release shape binary")
    parser.add_argument("--seeds", required=True, help="read-only seeds directory")
    parser.add_argument("--out", required=True, help="output directory")
    args = parser.parse_args()

    shape = os.path.abspath(args.shape)
    seeds = os.path.abspath(args.seeds)
    out = os.path.abspath(args.out)
    raw_out = os.path.join(out, "raw")
    os.makedirs(raw_out, exist_ok=True)

    files = sorted(
        name
        for name in os.listdir(seeds)
        if os.path.isfile(os.path.join(seeds, name))
    )

    manifest = []
    for name in files:
        path = os.path.join(seeds, name)
        lower = name.lower()
        if lower.endswith(".raw.js"):
            kind = "raw-js"
        elif lower.endswith(".js"):
            kind = "derived-js"
        elif lower.endswith(".json"):
            kind = "non-js-json"
        else:
            kind = "other"
        manifest.append(
            OrderedDict(
                [
                    ("name", name),
                    ("kind", kind),
                    ("size", os.path.getsize(path)),
                    ("sha256", sha256_file(path)),
                ]
            )
        )

    with open(os.path.join(out, "manifest.json"), "w") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")

    baseline = OrderedDict()
    proposed = OrderedDict()
    all_rows = {}

    for entry in manifest:
        name = entry["name"]
        path = os.path.join(seeds, name)
        if entry["kind"] not in ("raw-js", "derived-js"):
            continue
        run = run_ops_list(shape, path)
        with open(os.path.join(raw_out, name + ".ops-list.json"), "w") as handle:
            if run["status"] == "ok":
                json.dump(run["payload"], handle, indent=2)
            else:
                json.dump(
                    {"status": run["status"], "error": run.get("error")},
                    handle,
                    indent=2,
                )
            handle.write("\n")
        result, rows = summarize_source(name, entry["kind"], path, run)
        baseline[name] = result
        if rows is not None:
            proposed[name] = result
            all_rows[name] = rows

    # Baseline/proposed split keeps the exact same per-source structure; the
    # baseline file emphasizes current values, the proposed file the diff.
    with open(os.path.join(out, "baseline.json"), "w") as handle:
        json.dump(baseline, handle, indent=2)
        handle.write("\n")
    with open(os.path.join(out, "proposed.json"), "w") as handle:
        json.dump(proposed, handle, indent=2)
        handle.write("\n")

    # STOP decision per the assigned threshold: >20% classification-string
    # changes on any measured source.
    raw_results = [
        (name, result)
        for name, result in baseline.items()
        if result.get("source_kind") == "raw-js" and result.get("status") == "ok"
    ]
    stop_sources = [
        (name, result["changed_percent"]["classification"])
        for name, result in raw_results
        if result["changed_percent"]["classification"] > 20.0
    ]

    summary = OrderedDict(
        [
            ("shape_binary", shape),
            ("seeds_dir", seeds),
            ("seed_count", len(manifest)),
            (
                "seed_manifest",
                [{"name": e["name"], "kind": e["kind"], "sha256": e["sha256"], "size": e["size"]} for e in manifest],
            ),
            ("measured_raw_js", [name for name, _ in raw_results]),
            (
                "measured_derived_js",
                [
                    name
                    for name, result in baseline.items()
                    if result.get("source_kind") == "derived-js"
                ],
            ),
            ("stop_threshold_percent", 20.0),
            ("stop_sources", stop_sources),
            ("stop_fired", bool(stop_sources)),
            (
                "per_source",
                OrderedDict(
                    (
                        name,
                        OrderedDict(
                            [
                                ("status", result.get("status")),
                                ("handler_count", result.get("handler_count")),
                                ("preserved_projection_sha256", result.get("preserved_projection_sha256")),
                                ("current_inventory", result.get("current_inventory")),
                                ("proposed_inventory", result.get("proposed_inventory")),
                                ("changed", result.get("changed")),
                                ("changed_percent", result.get("changed_percent")),
                            ]
                        ),
                    )
                    for name, result in baseline.items()
                ),
            ),
        ]
    )
    with open(os.path.join(out, "summary.json"), "w") as handle:
        json.dump(summary, handle, indent=2)
        handle.write("\n")

    # Human-readable console report.
    print("Manifest (read-only seeds):")
    for entry in manifest:
        print("  %-8s %10d  %s  %s" % (entry["kind"], entry["size"], entry["sha256"], entry["name"]))
    print()
    for name, result in baseline.items():
        print("== %s (%s)" % (name, result["source_kind"]))
        if result["status"] != "ok":
            print("  STATUS %s: %s" % (result["status"], result.get("error")))
            continue
        print(
            "  handlers=%d current inventory=%s"
            % (result["handler_count"], result["current_inventory"])
        )
        print("  preserved non-derived projection sha256=%s" % result["preserved_projection_sha256"])
        print("  current class=%s" % result["current_classification"])
        print("  proposed class=%s inventory=%s" % (result["proposed_classification"], result["proposed_inventory"]))
        print(
            "  changed: class %d (%.3f%%), level %d (%.3f%%), proof %d, evidence %d, family %d"
            % (
                result["changed"]["classification"],
                result["changed_percent"]["classification"],
                result["changed"]["confidence_level"],
                result["changed_percent"]["confidence_level"],
                result["changed"]["proof_rule_id"],
                result["changed"]["evidence"],
                result["changed"]["semantic_family"],
            )
        )
        print("  baseline cross-tab (recognized|unknown -> classification):")
        for key, counts in result["baseline_cross_tab"].items():
            print("    %-14s %s" % (key, counts))
        print("  proposed cross-tab:")
        for key, counts in result["proposed_cross_tab"].items():
            print("    %-14s %s" % (key, counts))
        print()

    print("STOP threshold %.0f%%; fired=%s; sources over threshold=%s"
          % (summary["stop_threshold_percent"], summary["stop_fired"], summary["stop_sources"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
