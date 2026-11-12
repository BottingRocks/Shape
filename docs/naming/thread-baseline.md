# Thread naming baseline

Latest full measurement: **baseline130-005**, on the same 130 seeds
(13 vendors, ten seeds each, including Puma and Starbucks).

**331/38,345 functions remain unnamed (0.863216%)**, down from 381.
Separately, **207/919 native classes remain unnamed (22.524483%)**, down from 497.
That is **340 additional named bindings: 50 functions and 290 classes**, with
zero prior applied names lost or changed. The original 110 contribute 271 gains;
the added Puma/Starbucks seeds contribute 69.

All 130 inputs passed fresh scoped-reference, syntax, census, exact edit-replay
and registry-origin checks. The run preserved 1,859 wave names, 430 hook names,
the nine font names and three anchors. It reused the validated frozen debug
binary and the existing build cache with zero rebuilds.

[Per-seed counts](baseline130-005.csv). The per-seed report and the
identified sources were local run outputs and are not kept in the repository.

Frozen binary SHA-256: `d582579777e1cf7a0b5738eaea8b2b54c5842da142ca6a05a88147a6989cc95b`.

## Historical measurements

Measured 2026-11-01 on the fixed 110-seed selection (11 vendors, 10 seeds each).

**2,792 / 32,602 original generated function bindings remain unnamed (8.56%).** The previous baseline was 3,254 (9.98%): **462 additional function bindings named**, with no previous applied name lost or changed. The denominator is unchanged; removed empty stubs and classes are not counted as gains.

The run reused the corrected cleaned inputs and reran naming only. All 110 outputs passed scoped-reference preservation and exact byte-edit replay. Independent verification passed 1,927 checks; 47 focused tests cover the six new shared-rule modules.

Frozen binary SHA-256: `59b48aca9c9cd4b17e785bd7ffdffe73ed7d33ee7efebb04f54ec0bca0f5adbb`. Naming plus the reference audit took 131.66 seconds. Raw seed inputs, generated outputs, maps, and binary artifacts stay outside Git; the local report is `reports/thread-shared-rules-post110/baseline.md`.

## Later incremental checkpoint

A later checkpoint adds 12 named bindings across four cached seed checks, with no previous applied name lost or changed. It covers canvas random-word coercion variants, persistent transaction-state initialization, and the custom-event exchange bootstrap. All 26 focused tests pass. Independent verification checked 17,776 bindings and 90,432 references and replayed every reported edit exactly.

This subset is not a new full-corpus measurement: **2,792 / 32,602 remains the last verified 110-seed baseline**. The incremental binary SHA-256 is `35622d82dc0d3f40f51956f613e8fdcdfd5dc26cedab537f9658a3987a2119c6`. The local per-seed report is `reports/thread-shared-rules-post110/post2792-incremental.md`.

The next checkpoint covers six cumulative packages: the first three plus reordered signal snapshots, postliteral sentinel-extension members, and inline accessor descriptor lookups. Its four checked seeds have **21 additional names against the full baseline**, with 54 focused tests passing and no previous applied names lost or changed. Independent verification checked 16,220 bindings and 83,454 references. All six test receipts are actual raw Cargo output. Binary SHA-256: `8b392919417a7e8c90341469724d704182670e4b7dba56c4e4041f70e52e54bc`. Each incremental table compares its own seed subset against the full baseline above.

Checkpoint three adds inline method lookups, exact sentinel specialization, and scope-safe concrete names for independently identified duplicate functions. Its nine cumulative packages pass 91 focused tests and 76 allocator compatibility tests. Independent checks using one frozen binary on three representatives confirm 23 cumulative names against the full baseline, with no previous applied names lost or changed; four older outputs were audited separately as historical cases. This is another subset measurement, so the full 110-seed count above remains unchanged. Binary SHA-256: `c1b7871ad238b59e7e43fde13c4d27ff615fbdc5ab2ccbf98f3b374d2e885554`.

Checkpoint four adds assembled observer callbacks, navigation submission hooks, and UI-event sample variants. Its twelve cumulative packages pass 124 focused tests; the 76 allocator compatibility checks still apply. Three final-binary representatives have 23 cumulative names against their own full-baseline rows, with no baseline names lost or changed. Independent verification checked 12,577 bindings and 64,137 references. These three representatives differ from the previous checkpoint's cohort, so the subset gains are not additive. Binary SHA-256: `e0a6194cde939b2c00f2c4ca57885a1fc120c1bda5de88610be23bb2e1b893fd`.

Checkpoint five adds hash-fold and mixed numeric-index variants plus Observable-factory lowering support. Its fifteen cumulative packages have 165 focused test successes. On the same three representatives as checkpoint four it adds exactly four names, for 27 cumulative names against those full-baseline rows; all earlier applied names and references are preserved. The Observable rule's selected real witness remains unnamed despite its 17 portable tests passing, and a further source correction is pending. Binary SHA-256: `8c302d83e5477842b031d47e08ef58d52ffa6e3beff69980a9737aefdb9a63ab`.

## By vendor

| Vendor | Before unnamed | Now unnamed | Functions | Now unnamed % | Added names |
|---|---:|---:|---:|---:|---:|
| Chase | 402 | 336 | 3307 | 10.16% | 66 |
| GEICO | 190 | 163 | 2784 | 5.85% | 27 |
| Nordstrom | 265 | 250 | 2910 | 8.59% | 15 |
| Peacock | 281 | 237 | 3087 | 7.68% | 44 |
| PGE | 279 | 232 | 3099 | 7.49% | 47 |
| Southwest | 300 | 252 | 3073 | 8.20% | 48 |
| Target | 177 | 162 | 2958 | 5.48% | 15 |
| TDBank | 306 | 261 | 2994 | 8.72% | 45 |
| UHC | 391 | 323 | 3162 | 10.22% | 68 |
| USPS | 239 | 220 | 2042 | 10.77% | 19 |
| Xfinity | 424 | 356 | 3186 | 11.17% | 68 |

## Shared-rule gains

These count named bindings, including copies in different seeds/scopes, rather than distinct label strings.

| Role | Added names |
|---|---:|
| writeNumber | 168 |
| installMethodInterceptor | 70 |
| installAccessorInterceptor | 67 |
| startCanvasPixelProbe | 49 |
| handleCanvasProbeImageLoad | 49 |
| createSignalSerializer | 30 |
| encodeSignalBundle | 29 |

## Per seed

| Seed | Before unnamed | Now unnamed | Functions | Now unnamed % | Added names |
|---|---:|---:|---:|---:|---:|
| chase-wasm01 | 46 | 40 | 324 | 12.35% | 6 |
| chase-wasm02 | 40 | 34 | 334 | 10.18% | 6 |
| chase-wasm03 | 43 | 35 | 334 | 10.48% | 8 |
| chase-wasm04 | 40 | 32 | 330 | 9.70% | 8 |
| chase-wasm05 | 36 | 30 | 332 | 9.04% | 6 |
| chase-chase-more06 | 36 | 30 | 332 | 9.04% | 6 |
| chase-chase-more07 | 39 | 33 | 324 | 10.19% | 6 |
| chase-chase-more08 | 34 | 28 | 332 | 8.43% | 6 |
| chase-chase-more09 | 43 | 35 | 327 | 10.70% | 8 |
| chase-chase-more10 | 45 | 39 | 338 | 11.54% | 6 |
| geico-geico-final01 | 21 | 18 | 276 | 6.52% | 3 |
| geico-geico-final02 | 19 | 16 | 283 | 5.65% | 3 |
| geico-geico-final03 | 23 | 20 | 282 | 7.09% | 3 |
| geico-geico-final04 | 22 | 19 | 272 | 6.99% | 3 |
| geico-geico-final05 | 16 | 13 | 277 | 4.69% | 3 |
| geico-geico-more06 | 19 | 17 | 278 | 6.12% | 2 |
| geico-geico-more07 | 14 | 13 | 286 | 4.55% | 1 |
| geico-geico-more08 | 19 | 16 | 277 | 5.78% | 3 |
| geico-geico-more09 | 18 | 15 | 279 | 5.38% | 3 |
| geico-geico-more10 | 19 | 16 | 274 | 5.84% | 3 |
| nordstrom-nordstrom-final01 | 27 | 25 | 290 | 8.62% | 2 |
| nordstrom-nordstrom-final02 | 25 | 24 | 287 | 8.36% | 1 |
| nordstrom-nordstrom-final03 | 25 | 24 | 298 | 8.05% | 1 |
| nordstrom-nordstrom-final04 | 29 | 26 | 289 | 9.00% | 3 |
| nordstrom-nordstrom-final05 | 30 | 29 | 290 | 10.00% | 1 |
| nordstrom-nordstrom-more06 | 28 | 27 | 287 | 9.41% | 1 |
| nordstrom-nordstrom-more07 | 24 | 23 | 300 | 7.67% | 1 |
| nordstrom-nordstrom-more08 | 29 | 26 | 290 | 8.97% | 3 |
| nordstrom-nordstrom-more09 | 26 | 25 | 291 | 8.59% | 1 |
| nordstrom-nordstrom-more10 | 22 | 21 | 288 | 7.29% | 1 |
| peacock-peacock-final01 | 26 | 22 | 312 | 7.05% | 4 |
| peacock-peacock-final02 | 24 | 20 | 306 | 6.54% | 4 |
| peacock-peacock-final03 | 34 | 30 | 316 | 9.49% | 4 |
| peacock-peacock-final04 | 27 | 23 | 309 | 7.44% | 4 |
| peacock-peacock-final05 | 28 | 22 | 309 | 7.12% | 6 |
| peacock-peacock-more06 | 31 | 25 | 306 | 8.17% | 6 |
| peacock-peacock-more07 | 30 | 24 | 306 | 7.84% | 6 |
| peacock-peacock-more08 | 27 | 23 | 311 | 7.40% | 4 |
| peacock-peacock-more09 | 28 | 25 | 308 | 8.12% | 3 |
| peacock-peacock-more10 | 26 | 23 | 304 | 7.57% | 3 |
| pge-pge-final01 | 34 | 28 | 314 | 8.92% | 6 |
| pge-pge-final02 | 30 | 24 | 312 | 7.69% | 6 |
| pge-pge-final03 | 23 | 18 | 311 | 5.79% | 5 |
| pge-pge-final04 | 29 | 25 | 310 | 8.06% | 4 |
| pge-pge-final05 | 28 | 22 | 316 | 6.96% | 6 |
| pge-pge-more06 | 27 | 23 | 304 | 7.57% | 4 |
| pge-pge-more07 | 27 | 23 | 317 | 7.26% | 4 |
| pge-pge-more08 | 26 | 22 | 304 | 7.24% | 4 |
| pge-pge-more09 | 23 | 19 | 307 | 6.19% | 4 |
| pge-pge-more10 | 32 | 28 | 304 | 9.21% | 4 |
| southwest-southwest-final01 | 27 | 23 | 303 | 7.59% | 4 |
| southwest-southwest-final02 | 31 | 27 | 308 | 8.77% | 4 |
| southwest-southwest-final03 | 34 | 28 | 308 | 9.09% | 6 |
| southwest-southwest-final04 | 27 | 23 | 303 | 7.59% | 4 |
| southwest-southwest-final05 | 38 | 34 | 314 | 10.83% | 4 |
| southwest-southwest-more06 | 31 | 27 | 302 | 8.94% | 4 |
| southwest-southwest-more07 | 34 | 28 | 314 | 8.92% | 6 |
| southwest-southwest-more08 | 25 | 21 | 308 | 6.82% | 4 |
| southwest-southwest-more09 | 24 | 18 | 307 | 5.86% | 6 |
| southwest-southwest-more10 | 29 | 23 | 306 | 7.52% | 6 |
| target-seed01 | 11 | 8 | 303 | 2.64% | 3 |
| target-seed02 | 21 | 20 | 297 | 6.73% | 1 |
| target-seed03 | 19 | 18 | 296 | 6.08% | 1 |
| target-seed04 | 20 | 19 | 289 | 6.57% | 1 |
| target-seed05 | 20 | 17 | 294 | 5.78% | 3 |
| target-seed06 | 18 | 15 | 293 | 5.12% | 3 |
| target-seed07 | 16 | 16 | 295 | 5.42% | 0 |
| target-seed08 | 15 | 14 | 296 | 4.73% | 1 |
| target-seed09 | 18 | 17 | 299 | 5.69% | 1 |
| target-seed10 | 19 | 18 | 296 | 6.08% | 1 |
| tdbank-tdb-final01 | 24 | 20 | 294 | 6.80% | 4 |
| tdbank-tdb-final02 | 31 | 28 | 304 | 9.21% | 3 |
| tdbank-tdb-final03 | 28 | 22 | 295 | 7.46% | 6 |
| tdbank-tdb-final04 | 32 | 28 | 299 | 9.36% | 4 |
| tdbank-tdb-final05 | 35 | 31 | 293 | 10.58% | 4 |
| tdbank-tdb-more06 | 27 | 21 | 300 | 7.00% | 6 |
| tdbank-tdb-more07 | 40 | 36 | 305 | 11.80% | 4 |
| tdbank-tdb-more08 | 30 | 26 | 301 | 8.64% | 4 |
| tdbank-tdb-more09 | 31 | 27 | 301 | 8.97% | 4 |
| tdbank-tdb-more10 | 28 | 22 | 302 | 7.28% | 6 |
| uhc-uhc-final01 | 36 | 28 | 312 | 8.97% | 8 |
| uhc-uhc-final02 | 42 | 34 | 313 | 10.86% | 8 |
| uhc-uhc-final03 | 53 | 47 | 313 | 15.02% | 6 |
| uhc-uhc-final04 | 37 | 29 | 319 | 9.09% | 8 |
| uhc-uhc-final05 | 34 | 28 | 328 | 8.54% | 6 |
| uhc-uhc-more06 | 40 | 34 | 314 | 10.83% | 6 |
| uhc-uhc-more07 | 34 | 28 | 312 | 8.97% | 6 |
| uhc-uhc-more08 | 45 | 37 | 318 | 11.64% | 8 |
| uhc-uhc-more09 | 34 | 28 | 315 | 8.89% | 6 |
| uhc-uhc-more10 | 36 | 30 | 318 | 9.43% | 6 |
| usps-usps-final01 | 23 | 23 | 204 | 11.27% | 0 |
| usps-usps-final02 | 24 | 22 | 203 | 10.84% | 2 |
| usps-usps-final03 | 24 | 23 | 203 | 11.33% | 1 |
| usps-usps-final04 | 24 | 23 | 206 | 11.17% | 1 |
| usps-usps-final05 | 22 | 20 | 204 | 9.80% | 2 |
| usps-usps-more06 | 23 | 22 | 203 | 10.84% | 1 |
| usps-usps-more07 | 25 | 22 | 203 | 10.84% | 3 |
| usps-usps-more08 | 24 | 21 | 207 | 10.14% | 3 |
| usps-usps-more09 | 26 | 23 | 204 | 11.27% | 3 |
| usps-usps-more10 | 24 | 21 | 205 | 10.24% | 3 |
| xfinity-xfinity-final01 | 32 | 26 | 318 | 8.18% | 6 |
| xfinity-xfinity-final02 | 40 | 32 | 315 | 10.16% | 8 |
| xfinity-xfinity-final03 | 44 | 38 | 318 | 11.95% | 6 |
| xfinity-xfinity-final04 | 48 | 43 | 317 | 13.56% | 5 |
| xfinity-xfinity-final05 | 48 | 40 | 327 | 12.23% | 8 |
| xfinity-xfinity-more06 | 50 | 42 | 318 | 13.21% | 8 |
| xfinity-xfinity-more07 | 42 | 34 | 317 | 10.73% | 8 |
| xfinity-xfinity-more08 | 43 | 38 | 314 | 12.10% | 5 |
| xfinity-xfinity-more09 | 40 | 34 | 325 | 10.46% | 6 |
| xfinity-xfinity-more10 | 37 | 29 | 317 | 9.15% | 8 |

Checkpoint 006 (`post2792`): runtime entry, inline vector-copy and nested orientation variants add 11 names over checkpoint 005 on the same three inputs: GEICO 6/276 (2.17%), Xfinity more06 26/318 (8.18%), Nordstrom 15/290 (5.17%). Independent audit passed 136 checks, preserving all existing names and checked references; 45 new focused tests passed (210 cumulative). The 76-test compatibility receipt is historical only for this snapshot. The full 110-seed baseline remains 2,792/32,602 (8.56%); no broad rerun.

Checkpoint 007 (corrected source snapshot): collector-start, mutation-style and exchange-teardown rules add five names over checkpoint 006 on the same three inputs: GEICO 5/276 (1.81%), Xfinity more06 23/318 (7.23%), Nordstrom 14/290 (4.83%). Independent correction audit passed 134 checks; 43 new focused checks passed (253 cumulative), with no names lost or changed. The original snapshot failed because it pinned pre-glue exchange source; the corrected snapshot pins the tested reference-only fix and retains the original failure. Full110 baseline remains 2,792/32,602 (8.56%); no broad rerun.

Corrected checkpoints 008–009: frame-carrier, captured-field, seeded-emitter, observable, registry and entropy rules add six names over corrected checkpoint 007 on the common three inputs: GEICO 4/276 (1.45%), Xfinity more06 19/318 (5.97%), Nordstrom 13/290 (4.48%). Corrected009 passed 597 independent checks with 319 cumulative focused executions (66 new since007); no existing names lost or changed. The installation manifest also lists the already-audited captured-field source explicitly. The same binary passed the 11-anchor checkpoint: 105/3,250 unnamed (3.23%), versus181/3,250 at checkpoint004,76 additional names and0 losses. This is a representative cohort, not a110-seed rerun; global baseline remains2,792/32,602 (8.56%).

Checkpoint 010: collector-start inline-error, request-envelope and snapshot microvariants passed 33 focused checks (352 cumulative) and 719 independent audit checks. The three rule witnesses gained one name each: UHC03 24→23/313 (7.35%), Xfinity10 14→13/317 (4.10%), GEICO09 3→2/279 (0.72%), with no lost names. The common three-seed checkpoint remains 4/276, 19/318 and 13/290 (49 cumulative gains); separate witness gains are not added to that cohort. Compatibility 78 passed before the snapshot package and remains scoped to that earlier epoch. No 110-seed rerun.


Checkpoint 011: ICE-candidate seeded emitters, media bisection lowerings and AES-GCM IV-buffer framing passed 46 new focused checks (398 cumulative executions) and 1,065 independent checks. On the common three inputs, unnamed counts are GEICO 4/276 (1.45%), Xfinity more06 18/318 (5.66%) and Nordstrom 12/290 (4.14%): 51 cumulative gains versus the full baseline, two more than checkpoint 010, with no existing names lost or changed. Fresh static parsing verifies all 12,577 bindings and 64,137 references on these three outputs. Separate canonical no-label witnesses also preserve prior names: PGE09 10 to 9/307 and GEICO02 5 to 3/283; these are separate measurements, not added to the common cohort. The full 110-seed baseline remains 2,792/32,602 (8.56%); no broad rerun.


Checkpoint 012: statement-form queue defaults and cookie plugin/adapter variants passed 27 focused checks (425 cumulative executions) and the 15-check independent delta audit. The same Chase01 input gained seven queue names and two cookie names, moving from 21 to 12 unnamed out of 324 (3.70%), with no prior names lost or changed. Fresh static parsing across the three checkpoint011 outputs and this Chase012 witness checked 17,103 bindings and 87,689 references; all declaration targets were preserved. The common three-seed cohort remains 4/276, 18/318 and 12/290 (51 cumulative gains). This does not update the full 110-seed baseline; no broad rerun.


Checkpoint 013: the URL query-merge helper passed six focused checks, plus refreshed ICE (9) and cookie (8) targets. Nordstrom01 gains mergeUrlSearchParameter, moving from 12 to 11 unnamed out of 290 (3.79%); no prior names changed or disappeared. The common cohort is GEICO 4/276, Xfinity more06 18/318 and Nordstrom 11/290: 52 cumulative gains versus the full baseline. A fresh static audit of the changed Nordstrom output preserved all 4,119 bindings and 21,397 references; unchanged GEICO/Xfinity outputs reuse their exact input/output/edit proofs. The independent delta audit accepted the source changes; the manager copy adds the two omitted, already-verified refresh logs without changing source or binary bytes and retains the original audit/caveat. Full110 baseline remains 2,792/32,602 (8.56%); no broad rerun.


Checkpoint 014: worker-environment bundles, observable lowerings and WebRTC constructor branches passed 32 focused checks (5/20/7; 463 cumulative accepted-package executions). Independent checks accepted the source changes, name preservation and references. Versus checkpoint 013, GEICO01 moves from 4 to 2/276 (0.72%), Xfinity more06 from 18 to 16/318 (5.03%), and Nordstrom remains 11/290 (3.79%): four additional names and 56 cumulative gains in this fixed three-seed cohort, with no prior names lost or changed. All 12,577 bindings and 64,137 references were rechecked. Captured Target01 (1/303, 0.33%) and Peacock01 (3/312, 0.96%) witnesses also passed fresh static checks (8,585 bindings and 43,711 references), but lack individual binary-SHA receipts; they remain separate observations. A corrected side manifest pins 26 omitted witness, reference and label receipts; source and binary bytes are unchanged and the original audit caveats remain. Persistence003 and numeric002 failed compilation, were restored and are excluded; corrections004/003 are pending execution. The full 110-seed baseline remains 2,792/32,602 (8.56%); no broad rerun.


Checkpoint 015: seeded field emitters, tuple-mediated numeric stores and captured-native platform hooks passed 42 focused checks (19/17/6; 505 cumulative accepted-package executions). The independent 31-check delta audit accepted the source changes. Common-cohort unnamed counts move from GEICO01 2 to 1/276 (0.36%), Xfinity more06 16 to 13/318 (4.09%) and Nordstrom01 11 to 8/290 (2.76%): seven additional names and 63 cumulative gains versus the same full-baseline inputs, with no prior names lost or changed. Separate USPS01 moves from 5 to 4/204 (1.96%); Chase01 moves from 10 to 6/324 (1.85%), comprising one seeded-emitter and three platform-hook names. Fresh static checks on these five outputs preserve all 19,836 bindings and 103,228 references. The corrected side manifest adds 62 verified receipt pins without changing source or binary bytes. Individual witness/common run folders lack exact command/binary-SHA receipts; their binary context remains producer-attested, while the output/map integrity checks passed. The original failed platform comparison is retained; final comparisons show no name loss. This is a scoped checkpoint; the full 110-seed baseline remains 2,792/32,602 (8.56%), with no broad rerun.


### Checkpoint 016: 11 anchors, independently accepted

Persistence, beacon-report mapping and exchange-teardown listener variants passed 25 focused checks (2/10/13; 530 cumulative accepted-package executions). The independently reviewed frozen profile has 45 accepted packages. The same 11 anchor inputs now have **46 unnamed functions out of 3,250 (1.42%)**, versus 105 (3.23%) at checkpoint 009: **59 accumulated additional names across the intervening batches**, and 215 versus the same inputs in the full baseline. No existing applied name was lost or changed. The fixed three-input comparison gains three names specifically in batch 016 (GEICO01 1/276, Xfinity more06 12/318, Nordstrom01 6/290; 66 cumulative gains versus the full-baseline inputs).

| Anchor seed | Unnamed / functions | Unnamed |
|---|---:|---:|
| Chase01 | 6/324 | 1.85% |
| GEICO01 | 1/276 | 0.36% |
| Nordstrom01 | 6/290 | 2.07% |
| Peacock01 | 1/312 | 0.32% |
| PGE01 | 5/314 | 1.59% |
| Southwest01 | 4/303 | 1.32% |
| Target01 | 1/303 | 0.33% |
| TD Bank01 | 7/294 | 2.38% |
| UHC01 | 7/312 | 2.24% |
| USPS01 | 2/204 | 0.98% |
| Xfinity01 | 6/318 | 1.89% |

All 12 runs (11 anchors plus the Xfinity more06 comparison input) used debug binary `863a6cb24ebad3e45e298844558fe1cdcc33c125ed28294f34c9c3e362634c9e`, with individual command, binary, input, map and output receipts. They took 14.451 seconds at concurrency two. Only Target used its existing 75 labels. Static proofs preserved all 50,325 bindings and 258,609 references, and the independent reviewer verified those proofs against the actual artifacts. Class bindings are excluded from the function denominator; removed empty declarations are accounted for separately.

The parent015 reference in the producer profile is its original manifest rather than the side copy with 62 extra receipt pins; product bytes are identical and the review compared the corrected parent directly. The persistence module was first explicitly pinned in 016, closing the earlier 015 pin gap. Beacon source deliberately includes the previously accepted worker sweep alongside the new reviewed hunks. These are provenance notes, with no blocking findings. The full 110-seed baseline remains **2,792/32,602 (8.56%)**; it has not been rerun and these subset gains are not extrapolated globally.


### Checkpoint 017: focused updates after the 11-anchor checkpoint

Mouseup extracted-method calls, pure-IIFE hash folds and callable placeholder records passed 42 focused checks (11/14/17; 572 cumulative accepted-package executions). The independent audit passed all 39 checks with no blockers. The fixed three-input comparison gains one name in Xfinity more06 (12 to 11); its cumulative gain is 67 versus the same full-baseline inputs. Separate UHC01 gains hashStringOrBuffer (7 to 6) and USPS01 gains inspectAndDeleteFunctionPrototype (2 to 1). All prior names are preserved.

| Focused seed | Unnamed / functions | Unnamed |
|---|---:|---:|
| geico-geico-final01 | 1/276 | 0.36% |
| xfinity-xfinity-more06 | 11/318 | 3.46% |
| nordstrom-nordstrom-final01 | 6/290 | 2.07% |
| uhc-uhc-final01 | 6/312 | 1.92% |
| usps-usps-final01 | 1/204 | 0.49% |

All five final outputs share debug binary `4a89349cd96b0f9706368807df1e5e61b83a1cc28d3ca42c51e30d7e3f8a1ff0`, with exact input/map/output/command receipts and no labels. The independent static audit reproduced the proofs for 19,740 bindings and 101,591 references. These five inputs differ from the checkpoint015 proof set, so the different reference totals are not a regression. Intermediate execution receipts retain their binary hashes; only the final binary itself is frozen. Callable source preserves the previous beacon and worker changes and omits an identical duplicate helper from the old packet. Hash tests required one Rust type-only array conversion, with every assertion retained.

Canvas004 is excluded after its 10-pass/1-fail replay-order test result; the source was restored and mouseup substituted. Its test-only005 correction belongs to a later epoch. The 11-anchor table above remains the checkpoint016 snapshot, not a new 11-seed run; the full 110 baseline remains 2,792/32,602 (8.56%). No broad rerun.


### Checkpoint 018: canvas, metadata setters and seeded emitter variants

The three accepted packages passed 47 focused tests (11 canvas, 11 metadata setter and 25 seeded emitter; 619 cumulative accepted-package executions). Independent review passed all 47 checks with no blockers. The fixed three-input comparison improved from 18 to 11 unnamed functions out of 884: seven additional names, and 74 cumulative names versus these same inputs in the full baseline. No previous names were lost or changed.

| Focused seed | Unnamed / functions | Unnamed |
|---|---:|---:|
| geico-geico-final01 | 0/276 | 0.00% |
| xfinity-xfinity-more06 | 6/318 | 1.89% |
| nordstrom-nordstrom-final01 | 5/290 | 1.72% |

Separate final-epoch witnesses reached zero unnamed functions on Target01 (0/303), and improved Chase01 from 6 to 4/324 (1.23%). Target used its existing 75 labels; the other inputs used none. Both zero results (GEICO01 and Target01) apply to those inputs only. The five final outputs share debug binary `3d198e1a84a3bb26e68beb6fc94e19c7e642b8122d93ca76f1359c086ed484a1`; static proofs preserve 21,427 bindings and 110,279 references.

The unsuccessful punycode001 and envelope-forwarder001 tests were restored and excluded. The complete 11-anchor table remains checkpoint016, and the full 110-seed baseline remains 2,792/32,602 (8.56%). Neither cohort was rerun for this checkpoint.


### Checkpoint 019: storage removal, punycode and envelope forwarders

The three accepted packages passed 39 focused tests (21 storage, 6 punycode and 12 envelope; 658 cumulative accepted-package executions). Independent review passed all 56 checks with no blockers. The fixed three-input comparison improved from 11 to 10 unnamed functions out of 884; its cumulative gain is 75 versus these same inputs in the full baseline. Separate UHC01 improved from 5 to 4 and Chase01 from 4 to 3. This batch added three names across the five inputs, with no previous names lost or changed.

| Focused seed | Unnamed / functions | Unnamed |
|---|---:|---:|
| geico-geico-final01 | 0/276 | 0.00% |
| xfinity-xfinity-more06 | 5/318 | 1.57% |
| nordstrom-nordstrom-final01 | 5/290 | 1.72% |
| uhc-uhc-final01 | 4/312 | 1.28% |
| chase-wasm01 | 3/324 | 0.93% |

All five final outputs share debug binary `2ea46064da833e87e0f66a3a1ba2ba3c934c82cff6f533ba72393f4fda73f1c7`, with exact command/input/map/output receipts and no labels. Static proofs preserve 21,533 bindings and 109,604 references. This cohort uses UHC instead of checkpoint018’s Target, so the different reference totals are not a regression.

Both independent p07 patches share one compiled file. Exact reconstruction proofs recover each reviewed predecessor separately; the helper retains its mandatory byte checks. The six punycode tests ran before the envelope merge, with their original source/binary epoch retained; final merged behavior was checked on the five inputs. No test log was retagged. The audit records one non-blocking alias-path labeling nit; the correct original review and hash remain pinned. Curried004’s zero-gain attempt is excluded.

The complete 11-anchor table remains checkpoint016. The full 110-seed baseline remains 2,792/32,602 (8.56%); neither cohort was rerun for this checkpoint. Newer work in the compiler checkout is outside this committed snapshot.

### Checkpoint 020: eleven anchors, string joins and snapshot variants

Code-unit joins, compact/reverse seeded emitters and split-filter signal snapshots passed **59 focused checks** (15/30/14; 717 cumulative executions). The frozen profile has 57 accepted packages. Independent review passed **58/58 checks** with no blockers, pinning 102 compiled files and 2,044 auxiliary artifacts.

The eleven anchors have **24 unnamed functions out of 3,250 (0.74%)**, down from 46 (1.42%) at checkpoint016: **22 accumulated additional names across checkpoints017–020**, with no applied names lost or changed. The extra Xfinity more06 input has 5/318 unnamed (1.57%); all twelve inputs total 29/3,568 (0.81%).

| Final seed | Unnamed / original functions | Unnamed % |
|---|---:|---:|
| chase-wasm01 | 2/324 | 0.62% |
| geico-geico-final01 | 0/276 | 0.00% |
| nordstrom-nordstrom-final01 | 5/290 | 1.72% |
| peacock-peacock-final01 | 0/312 | 0.00% |
| pge-pge-final01 | 4/314 | 1.27% |
| southwest-southwest-final01 | 2/303 | 0.66% |
| target-seed01 | 0/303 | 0.00% |
| tdbank-tdb-final01 | 5/294 | 1.70% |
| uhc-uhc-final01 | 1/312 | 0.32% |
| usps-usps-final01 | 1/204 | 0.49% |
| xfinity-xfinity-final01 | 4/318 | 1.26% |
| xfinity-xfinity-more06 | 5/318 | 1.57% |

All twelve outputs use final debug binary e0b40049ab149a372b5365858c54fab4f0da741b81c587af6f4090c3cf45fb27. Static checks preserve **50,325 bindings and 258,609 references**. Only Target uses its existing75-label reference; every other input uses no labels. The focused targets ran on separately pinned stage binaries; the final twelve outputs share the final binary.

The individual package witnesses gained four names (Southwest1, UHC2, UHC1). The fixed GEICO/Xfinity-more06/Nordstrom cohort stays at 10/884, unchanged from checkpoint019. On the known-five checkpoint019 comparison, UHC4→1 and Chase3→2 account for four gains; these overlapping measurements are not summed. The full110-seed baseline remains **2,792/32,602 (8.56%)**; it was not rerun.

Frozen manifest SHA-256: 493c360b1b7a00fcd935b3bad1031a49d21fae6e34f71810c0e7582cc8d68cb5. Failed/stale attempts remain excluded evidence. Later checkpoint021 USPS/UHC zero witnesses are separate from this committed020 snapshot.

### Checkpoint 021: curried emitters, metadata setters and observables

The three packages passed **51 focused checks** (10/13/28; 768 cumulative executions across 60 accepted packages). Independent review passed **68/68 checks**, with no blockers, pinning 104 compiled files and 2,518 auxiliary artifacts.

The eleven anchors improved from 24 to **17/3,250 unnamed (0.52%)**, seven additional names. The extra Xfinity more06 input gained one: all twelve improved from 29 to **21/3,568 (0.59%)**, eight additional names with no prior names lost or changed. Five checked anchors have zero remaining unnamed functions.

| Final seed | Unnamed / original functions | Unnamed % |
|---|---:|---:|
| chase-wasm01 | 1/324 | 0.31% |
| geico-geico-final01 | 0/276 | 0.00% |
| nordstrom-nordstrom-final01 | 4/290 | 1.38% |
| peacock-peacock-final01 | 0/312 | 0.00% |
| pge-pge-final01 | 4/314 | 1.27% |
| southwest-southwest-final01 | 1/303 | 0.33% |
| target-seed01 | 0/303 | 0.00% |
| tdbank-tdb-final01 | 4/294 | 1.36% |
| uhc-uhc-final01 | 0/312 | 0.00% |
| usps-usps-final01 | 0/204 | 0.00% |
| xfinity-xfinity-final01 | 3/318 | 0.94% |
| xfinity-xfinity-more06 | 4/318 | 1.26% |

The fixed GEICO/Xfinity-more06/Nordstrom cohort improved from 10 to 8/884. Eight measured gains comprise five createObservableClass, two setSelectMetaProbeValue and one emitSeededHashEncoding applications: three package witnesses plus five transfers, rather than eight independent families. Static proofs preserve **50,325 bindings and 258,609 references** across the twelve outputs. Only Target uses the existing75-label reference; all others use no labels.

Final binary: 095e02f87687d5091d168c9dd40129e0d6a5ff8d38d83cb662018e93da9027e1. The focused targets ran on their separately pinned stage binaries. The curried packet core b5c15b41 was never compiled: the actual core5f056a56 adds only its module declaration and fallback after storage to parent52f268a2, with exact reverts to both historical versions. The observable005 test changes exactly three invalid alpha-name literals from suffix1 to2; source is unchanged from reviewed004. The failed004 test run remains excluded.

Frozen manifest SHA-256: b01566db24ffb9f52a2133c8a273ffbdb96bc20b5fbbb536e6ef817478936a7a. The full110-seed baseline remains **2,792/32,602 (8.56%)** and was not rerun. Newer checkpoint022 Chase/Southwest zero witnesses and the next Nordstrom gain are separate from this committed021 snapshot.

### Checkpoint 022: repeated keyup registration, storage and beacon records

Three shared variants passed **55 focused checks** (8/32/15; 823 cumulative executions across 63 accepted packages). Independent review passed **58/58 checks** with no blockers, pinning 106 compiled files and 3,036 auxiliary artifacts. The six changed products comprise four updates and two new focused tests.

The eleven anchors improved from **17 to 14 unnamed out of 3,250 (0.43%)**, with seven anchors at zero. All twelve inputs, including Xfinity more06, improved from 21 to **18/3,568 (0.50%)**. Exactly three functions gained names: recordKeyupHoldStatistics on Southwest, emitStorageRemovalSignal on Chase, and buildBeaconDescriptorPayloads on Nordstrom. No applied name was lost or changed.

| Final seed | Unnamed / original functions | Unnamed % |
|---|---:|---:|
| chase-wasm01 | 0/324 | 0.00% |
| geico-geico-final01 | 0/276 | 0.00% |
| nordstrom-nordstrom-final01 | 3/290 | 1.03% |
| peacock-peacock-final01 | 0/312 | 0.00% |
| pge-pge-final01 | 4/314 | 1.27% |
| southwest-southwest-final01 | 0/303 | 0.00% |
| target-seed01 | 0/303 | 0.00% |
| tdbank-tdb-final01 | 4/294 | 1.36% |
| uhc-uhc-final01 | 0/312 | 0.00% |
| usps-usps-final01 | 0/204 | 0.00% |
| xfinity-xfinity-final01 | 3/318 | 0.94% |
| xfinity-xfinity-more06 | 4/318 | 1.26% |

Static evidence preserves **50,325 bindings and 258,609 references** across the twelve outputs. The independent audit validated the saved static proofs and input/output/map context; it did not rerun the helper. Only Target used its existing 75 labels; all other inputs used no labels.

Final binary: 1369dde5633a946d2d94800807efac6f42882d02a52502d217e7c5a14ae67d3b. The raw beacon packet first failed to compile; three recorded type/borrow corrections produced the tested source, followed by all 15 focused tests passing. Exact reconstruction proofs preserve both the raw reviewed source and the prior committed source; no semantic predicate or assertion changed. The storage packet’s unused old shared core was excluded, preserving core5f056a56. Focused logs retain their separate execution epochs.

Frozen manifest SHA-256: 2526aa9f16e207d7d5afd2efed83c73cafdc3ce3f61fe406d8b7cc89060c4359. The full 110-seed baseline remains **2,792/32,602 (8.56%)** and was not rerun. These anchor results do not establish global zero. Newer checkpoint023 work remains outside this committed022 snapshot.

### Checkpoint 023: persistent helper flow, ready queue and observer normalization

Three additional packages passed **46 focused checks** (14/2/30; 869 cumulative executions across 66 accepted packages). Independent review passed **68/68 checks** with no blockers. It trusted the pinned checkpoint022 audit, checked the new package delta, and verified all 108 tracked compiled paths plus 3,552 auxiliary pins. The manifest adds p02_exec_a and its focused test to its tracked path set; p02_exec_a already existed in Git. This commit updates five existing product paths and adds one focused test.

The eleven anchors improved from **14 to 11 unnamed out of 3,250 (0.34%)**, with seven anchors at zero. All twelve checked inputs total **15/3,568 (0.42%)**, down from 18. The three gains are initializePersistentTransactionState on TDBank, flushReadyQueue on PGE, and createObservableClass on Xfinity. No previously applied name was lost or changed.

| Final seed | Unnamed / original functions | Unnamed % |
|---|---:|---:|
| chase-wasm01 | 0/324 | 0.00% |
| geico-geico-final01 | 0/276 | 0.00% |
| nordstrom-nordstrom-final01 | 3/290 | 1.03% |
| peacock-peacock-final01 | 0/312 | 0.00% |
| pge-pge-final01 | 3/314 | 0.96% |
| southwest-southwest-final01 | 0/303 | 0.00% |
| target-seed01 | 0/303 | 0.00% |
| tdbank-tdb-final01 | 3/294 | 1.02% |
| uhc-uhc-final01 | 0/312 | 0.00% |
| usps-usps-final01 | 0/204 | 0.00% |
| xfinity-xfinity-final01 | 2/318 | 0.63% |
| xfinity-xfinity-more06 | 4/318 | 1.26% |

All twelve outputs use binary 4016af716f0915c0502df1efd901a359504bb0c11b0afdc1dc8e77d7b992ea50. Eleven are fresh measurements; Xfinity final01 reuses its byte-identical witness after matching every binary/input/labels/output/map pin. Static evidence preserves **50,325 bindings and 258,609 references**; the independent review validated the saved proof context rather than rerunning the helper. Only Target used its existing 75 labels; all other seeds used none.

Focused logs retain their separate source/binary epochs. The failed font003 package remains excluded: ten initial compiler errors, followed by 2/11 passing after type corrections, then a full restore. Its authoritative restore receipts and absent-module snapshots supersede a stale restored=false field in an older failure note. The correction patch has five hunks and seven changed-line corrections; older seven-hunk wording is a metadata advisory. No failed run was counted as accepted.

Frozen manifest SHA-256: 2ea6457f2b9475bbc5243d50da3efaa3269c1e9eb2eaa3f8b5c721b5d42d1777. Historical ROOT714ebde4 capture remains historical; parent022 was independently verified and committed as c2c8650e. The full110 baseline remains **2,792/32,602 (8.56%)**, without a new full run or global extrapolation. Newer checkpoint024 gains are outside this committed023 snapshot.

### Checkpoint 024: saved stopping point

At the user’s request, work stopped at **7 unnamed / 3,250 functions (0.22%) across the eleven anchors**, down from 11. Eight anchors have zero unnamed functions. The additional Xfinity more06 seed has 3/318 (0.94%); all twelve checked seeds total **10/3,568 (0.28%)**. This is a measured anchor checkpoint, not a new full110 baseline.

Two accepted packages passed **79 focused checks** (beacon004 27/27 and storage006 52/52), bringing the recorded lineage to 68 accepted packages and 948 focused executions. Nordstrom gained three global-collection API names (3→0/290); Xfinity final01 gained emitStorageRemovalSignal (2→1/318). The extra Xfinity more06 input independently measured the same storage-family transfer (4→3/318). No previously applied name was lost or changed.

| Final seed | Unnamed / original functions | Unnamed % |
|---|---:|---:|
| chase-wasm01 | 0/324 | 0.00% |
| geico-geico-final01 | 0/276 | 0.00% |
| nordstrom-nordstrom-final01 | 0/290 | 0.00% |
| peacock-peacock-final01 | 0/312 | 0.00% |
| pge-pge-final01 | 3/314 | 0.96% |
| southwest-southwest-final01 | 0/303 | 0.00% |
| target-seed01 | 0/303 | 0.00% |
| tdbank-tdb-final01 | 3/294 | 1.02% |
| uhc-uhc-final01 | 0/312 | 0.00% |
| usps-usps-final01 | 0/204 | 0.00% |
| xfinity-xfinity-final01 | 1/318 | 0.31% |
| xfinity-xfinity-more06 | 3/318 | 0.94% |

The twelve outputs share accepted binary aa9d823a3fe1588a52e1b0b285ebe8f04a68fc663fa26ffb7d5a45106c83bc75. Static proof preserves **50,325 bindings and 258,609 references**. Only Target uses its existing 75 labels; all other cases use none. This early checkpoint contains two accepted packages, with the source delta and saved execution evidence reviewed before commit; it is not claimed as a separate independent full-prefix audit.

Failed seeded013, storage005 and seeded014 attempts were restored and excluded. The later font004 run passed 10/11, with one holder/push-alias positive failing; it was also restored and excluded. The incomplete seeded diagnostics and source-only candidate work remain in separate local workspaces. All active workers were stopped, and the compiler workspace was restored to the accepted108-product state and the accepted binary.

Final024 staging READY SHA-256: ec60dd7358a24da09c1ae6926091bddec585f6c928b9bb973760293d17e5f8d1. Final SEAL: 6def7f44feeeff4fd06cde2a1aa66aea0de0a634825ca5679e493c58705b6ead; 499 staged file hashes were verified. An earlier provisional seal was superseded while its producer was still finalizing restore-epoch metadata; only the terminal producer’s final seal was accepted. The full110 baseline remains **2,792/32,602**, without extrapolating these anchor gains. Further naming work is paused by user request.


### Bounded three-family pass, 2026-11-01

The same ten least-known seeds now have **68/2,998 unnamed (2.27%)**, down from86 immediately before this pass and90 before the prior anchor fixes transferred four names. Three shared families add18 names with zero prior-name losses/changes across15 checked seeds; nine font names and three repaired anchors are preserved. The primary focused targets pass111 tests plus11 existing emitter-agreement checks (122 total); all15 reference audits and byte-edit replays pass. The old full110 remains the frozen402/32,602 (1.23%) measurement; no global result is extrapolated from this subset.

The per-seed counts, final seed links and 90-function diagnostics were a local report that is not kept in the repository. The next requested full baseline is130seeds,10pervendor includingStarbucks andPuma; it has not started.


### Verified 130-seed baseline, 2026-11-01

The completed baseline has **768/38,345 unnamed (2.002869%)**, exactly 10 accepted captures per 13 vendors. On the unchanged original110 cohort, unnamed fell **402 → 343/32,602 (1.052083%)**: 59 additional names, zero prior applied-name losses or changes. Added20 is a first baseline: Puma **205/2,724 (7.525698%)** and Starbucks **220/3,019 (7.287181%)**, combined **425/5,743 (7.400313%)**. The expanded-cohort percentage is not a like-for-like regression.

All130 passed reference audits, exact edit replay and independent denominator census; all20 added captures passed scoped recovery checks with zero stops or unresolved decoder residue. Puma07 and Starbucks06 failed decoder validation and were replaced by ordered spare11; failure ledgers remain in the report. Latest15 outputs, nine font names, three repaired anchors and18 recent family gains are preserved. Three normal Sol medium workers reused one frozen binary/cache with zero build time.

The per-seed links, counts, gains and manager review were a local report that is not kept in the repository. This supersedes the earlier planned130 status; historical baseline records above remain unchanged.


### Verified positional-wave baseline130-003, 2026-11-01

The same 130 accepted seeds now have **408/38,345 unnamed (1.064024%)**, down from **481 (1.254401%)** in baseline130-002: **73 additional names**, with zero unexpected prior-name losses or changes. Original110 improved **272 → 241/32,602 (0.739218%)**, a gain of31; Puma improved **107 → 86/2,724 (3.157122%)**, and Starbucks **102 → 81/3,019 (2.683008%)**, 21 additional names each.

All **1,859/1,859 grouped registrations** use the name determined by their registry position: slot0 is waveCollector1. Eighteen previously unnamed generated wave functions and four anonymous identities were named; ten prior purpose labels intentionally moved to positional labels with their semantic observations retained as metadata. Anonymous identities are separate from generated-function gains. The 430 platform-hook origins are retained, with no wave-caused hook changes; the prior verified Nordstrom API gain remains intact.

The first full130 candidate failed preservation in Southwest03, USPS01, Xfinity-more10 and Puma03: 52 existing wave labels were lost or changed. That candidate was rejected. Combining the existing complete wiring proof with independent registry discovery restored all52; equal position claims deduplicate, while conflicting positions refuse. The corrected binary passed **293 focused tests across23 targets**, **21 focused witness audits**, and the fresh **130/130 reference, scope, census and exact byte-replay checks**. All nine font names and three repaired anchors are preserved. Only normal Sol medium workers and one build cache were used.

[Portable per-seed counts and hashes](baseline130-003.csv). In the local report, identified code links to renamed.js and original code with THREAD names links to cleaned.js; all260 exports and link targets were checked.

Frozen binary SHA-256: 92f73a51ca24392906eaf46dfe6ae87bdcaea51ebd593660a02f402c2d5fe033. Full validation and the retained failed-attempt history are linked from the new report. No gains are extrapolated from a subset.

### Verified function and class baseline130-004, 2026-11-01

The same 130 inputs now have **381/38,345 unnamed functions (0.993611%)**, down
from 408: **27 additional function names**, with no prior name or metadata losses.
Original110 improved **241 → 224/32,602 (0.687074%)**; Puma improved **86 →
82/2,724 (3.010279%)**, and Starbucks **81 → 75/3,019 (2.484266%)**.

Native class bindings are now counted separately. The previous function metric
excluded **919 generated class bindings** and their native method callables.
This pass names **422 class bindings**: 129 Observable, 104 Subscription,
104 SubscriptionObserver and 85 ValueEncoder. **497/919 classes remain unnamed
(54.080522%)**. Class labels and method metadata are separate from function gains.

All four naming packages passed **140 focused tests across ten suites** and the
fresh **130/130 scoped-reference, syntax, census, pruning and exact edit-replay
checks**. All **1,859 positional wave names**, **430 hook origins/names**, nine
font names and three repaired anchors were preserved. The full run reused one
frozen debug binary with zero builds. All 260 source exports and their report
links were checked; identified output is renamed.js and original THREAD input
is cleaned.js.

[Portable per-seed counts and links](baseline130-004.csv); the per-seed report
with identified sources was a local run output and is not kept in the repository.

Frozen binary SHA-256: dab1e88485982b854a1756dc1d35aac3cc528e791f11325f19e0538283bcf402.
