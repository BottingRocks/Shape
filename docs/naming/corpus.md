# Program naming corpus

`shape naming corpus` is the **durable replacement for the research target
lists and per-package coverage reports**. It runs the whole naming chain for
every seed, records the result as a deterministic fingerprint, and compares
fingerprints along three axes:

1. **temporal** — a reviewed golden baseline (`--baseline` /
   `--update-baseline`): did this seed regress since the last accepted review?
2. **intra-corpus** — one reference seed (`--reference`): is the naming plan
   overfit to one capture, and which roles/names does each seed lack?
3. **inter-vendor** — vendor groups (`--manifest`, `--vendor NAME=PATH`): which
   roles are resolved everywhere, which are vendor-only, and where do shared
   roles get different names?

No target JSONs, corpus environment variables, catalogs or per-package report
files are read by this command. A baseline is the only saved artifact.

## Pipeline

For each seed:

```text
raw capture --(trace)--> linked program --(plan)--> semantic names
            --(rename)--> renamed source --(annotate)--> annotated source
```

- A `*.raw.js` seed is traced with `shape trace program`'s library entry point
  (thread cleanup **on** by default; `--steps N` is the dispatch budget).
  `--no-thread-cleanup` disables the per-thread cleanup. `--trace` forces
  tracing for a seed whose name does not end in `.raw.js`.
- Any other seed is read as an already-linked program and goes straight to the
  naming plan.
- The plan is `plan_semantic_names`; the rename is
  `rename_semantic_names_with`; anonymous output is
  `apply_anonymous_names_mapped`. This is the same path as
  the internal semantic naming APIs; recovered JavaScript is never executed.
- Seeds are independent and processed in parallel on std threads.

`--out DIR` writes:

| file | when | contents |
| --- | --- | --- |
| `<stem>.program.js` | traced seeds only | the linked program |
| `<stem>.renamed.js` | always | the plan's renames, readable local names, and `--name-anonymous` names; `--no-local-names` keeps generated locals |
| `<stem>.names.json` | always | the `SemanticNamingPlan` exactly as `shape naming plan` prints it |
| `<stem>.annotated.js` | `--annotate-anonymous` | the renamed source with `/* <name> */` comments |

An existing output is refused unless `--force` is passed. No output may be the
seed itself.

## Fingerprint

Running the same command twice on the same inputs produces byte-identical JSON.
The schema is `program-naming-corpus/1`. A baseline is:

```json
{
  "version": "program-naming-corpus/1",
  "seeds": {
    "<label>": { "seed": { ... }, "plan": { ... }, "rename": { ... },
                 "anonymous": { ... }, "trace": { ... } }
  }
}
```

Per seed:

```json
{
  "seed": {
    "path": "kernel.raw.js",        // file name only: no absolute paths
    "sha256": "…", "bytes": 123456,
    "mode": "traced",                // or "linked"
    "label": "kernel.raw.js",
    "vendor": "acme",                // null when unlabelled
    "build": "7", "date": "2026-01-02"
  },
  "trace": {                         // traced seeds only
    "traced": 5,
    "stopped": [{"function": "d1@100/0", "reason": "…"}],
    "program": {"sha256": "…", "bytes": 98765},
    "kernel_globals": ["host", "…"]
  },
  "plan": {
    "candidates": 12,
    "anonymous": 3,
    "ambiguous": [{"role": "…", "reason": "…"}],
    "skipped": [{"role": "…", "kind": "refused", "reason": "…"}],
    "roles": {
      "agent_module": [
        {"entity_id": "fn@1..117",
         "proposed_name": "createAgentModule",
         "binding_kind": "function"}
      ]
    },
    "anonymous_entries": {
      "fn@50..90": {"role": "…", "proposed_name": "…", "arrow": false}
    }
  },
  "rename": {
    "applied": 10, "skipped": 2,
    "output": {"sha256": "…", "bytes": 1234},
    "parses": true
  },
  "anonymous": {
    "name_anonymous": false, "annotate_anonymous": false,
    "named": 0, "annotated": 0, "skipped": 0,
    "outcomes": {"name": [], "annotate": []}
  }
}
```

All maps and arrays are sorted by stable keys (`role`, then `entity_id`;
`vendor`, then `label`, then `role`). The fingerprint contains **no
timestamp, absolute path, host id or build id**. `build`/`date` are the
manifest's human-supplied metadata, not generated values. Trace facts
(`trace`) and naming facts (`plan`, `rename`, `anonymous`) are separate
objects so a reviewer sees which layer moved.

## Baseline semantics

A baseline is a **human-reviewed golden file**, then machine-diffed forever.
The first baseline per seed is reviewed once by a human; thereafter diffs are
regression tracking. CI runs `--baseline` without `--update-baseline`. Target
lists and per-package coverage reports are removed once baselines are
accepted.

- `--update-baseline` prints the per-seed diff it is accepting — added,
  removed and changed roles/names, refusals, ambiguities and trace stops —
  before writing. Accepting a baseline is a reviewable act, never silent. For
  a seed with no previous fingerprint it prints the full role → name map.
- `--baseline` compares without writing. It also reports a changed **seed**
  SHA-256 separately from a semantic regression, and still compares the
  naming layers of the changed input.

Regressions (exit 1):

| regression | example |
| --- | --- |
| a function traced before now stops | `function now stops: d1@100/0` |
| the traced function count falls / stopped count rises | `trace: traced count fell 5 -> 4` |
| a role that had a resolved candidate now has none | `role lost: agent_module` |
| a role's proposed name changes | `name changed: agent_module: createAgentModule -> makeAgentModule` |
| a new refusal or ambiguity appears | `new refusal: …`, `new ambiguity: …` |
| the renamed output no longer parses | `renamed output no longer parses` |

Reported, not failed:

- added roles/names, a higher traced count, fewer stops, resolved refusals or
  ambiguities (improvements);
- a changed program/renamed-output SHA-256 with identical role → name and stop
  sets (a `changed` line, not a regression);
- a changed input SHA-256 (reported separately, then the semantic layers are
  still compared);
- a seed in the baseline that is not in this run (`changed: not in this run`).

Exit codes:

| code | meaning |
| --- | --- |
| 0 | no baseline regression and no opted-in report failure |
| 1 | baseline regression, an opted-in `--fail-on` report failure, or a runtime error |
| 2 | usage error |

A traced seed whose functions stopped still exits 0 unless a baseline diff
says it regressed; the stops are part of the fingerprint.

## Intra-corpus: `--reference`

```sh
shape naming corpus a.raw.js b.raw.js c.raw.js --reference a.raw.js
```

Every other seed is compared with the reference: a per-role matrix marking a
role as `both`, `reference-only`, or `seed-only`, proposed-name differences on
shared roles, and coverage deltas (roles resolved per seed). This is the "not
overfit to one capture" check. Use it when adding a capture or changing the
plan; use the temporal baseline for "did this capture regress?".

`--format markdown` prints the matrix as a markdown table on stdout; the
default is a compact matrix on stderr. `--json` carries the same facts.

## Inter-vendor: `--manifest` and `--vendor`

Seeds may carry `vendor` and optional `label`, `build`, `date`. Paths in a
manifest are relative to the manifest file; a seed with no vendor is grouped
as `unlabelled`, not rejected.

```json
{
  "version": "program-naming-corpus/1",
  "vendors": {
    "acme": {"build": "7", "date": "2026-01-02", "seeds": ["acme/kernel.raw.js"]},
    "globex": {"seeds": [{"path": "globex/kernel.raw.js", "label": "globex-main"}]}
  },
  "seeds": ["shared/kernel.raw.js"]
}
```

```sh
shape naming corpus --manifest vendors.json
shape naming corpus --vendor acme=a.raw.js --vendor globex=b.raw.js
```

The vendor matrix reports roles resolved in **all** vendors, vendor-only
roles, and same-role-different-name divergences. Ordering is vendor, then
label, then role. A plan currently maps one role to one vocabulary name, so
name divergences normally appear only when the naming vocabulary itself
changes; missing/vendor-only roles are the common vendor signal.

## Report failures are opt-in

Comparison reports exit 0 by default. Add one or both flags with
`--reference` or vendor groups to make a report failure exit 1:

```sh
shape naming corpus --manifest vendors.json --fail-on missing-role
shape naming corpus --reference base.raw.js other.raw.js --fail-on name-divergence
```

- `--fail-on missing-role`: a role resolved in the reference but missing from
  a compared seed; in vendor mode, a role resolved in some vendor but not all.
- `--fail-on name-divergence`: a shared role whose proposed-name set differs
  between the reference and a seed, or between vendor groups.

## Examples

```sh
# First review and accepted baseline.
shape naming corpus captures/*.raw.js --out reviewed/ \
  --baseline tests/program-corpus-baseline.json --update-baseline

# CI: no writing, regressions exit 1.
shape naming corpus captures/*.raw.js --baseline tests/program-corpus-baseline.json

# Naming plan only, no outputs; prints the per-seed summary and role counts.
shape naming corpus linked/program.js --json

# Machine-readable result including baseline diff, aggregate and reports.
shape naming corpus --manifest vendors.json --baseline baseline.json --json
```

The `--json` document has `{version, command, status, result}`; `result`
contains `schema`, `seeds`, `diff`, `regressions`, `improvements`, `changes`,
`input_changes`, `baseline_updated`, `aggregate`, `reference` and `vendors`.
