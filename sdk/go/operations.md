# Go operation coverage

Authoritative wire IDs: [shared contract](../operations.json). Named operations
use v2 where policy differs; existing helpers for the original commands retain
their compatible request/results and legacy dispatch. `CallOperation` exposes
all v2 operations; `Call` remains the original dispatcher.

| Wire operation | Public Go API |
| --- | --- |
| `version` | `NativeVersion / Commands` |
| `deob` | `Deob` |
| `report` | `Report` |
| `kernel.show` | `KernelShow` |
| `ops.list` | `OpsList` |
| `ops.show` | `OpsShow` |
| `trace.program` | `TraceProgramWithOptions / TraceProgramWithArtifact / LoadedKernel.TraceProgram` |
| `trace.thread` | `TraceThread / LoadedKernel.TraceThread` |
| `cleanup.thread` | `CleanupThread` |
| `naming.threads` | `NamingThreads` |
| `naming.collectors` | `NamingCollectors` |
| `naming.plan` | `NamingPlan` |
| `naming.inventory` | `NamingInventory` |
| `signals.schema` | `SignalsSchema` |
| `signals.show` | `SignalsShow` |
| `signals.inputs` | `SignalsInputs` |
| `signals.contracts` | `SignalsContracts` |
| `pin` | `Pin` |
| `session.from_initializer` | `SessionFromInitializer` |
| `artifact.compile` | `CompileArtifactWithOptions / CompileLoadedArtifact` |
| `artifact.compact_schema` | `CompactSchema` |
| `signals.catalog` | `SignalsCatalog` |
| `signals.describe` | `SignalsDescribe` |
| `signals.compact_schema` | `SignalsCompactSchema` |
| `kernel.load` | `KernelDocument / LoadKernel` |
| `kernel.to_json` | `KernelJSON / LoadedKernel.JSON` |
| `ops.classify.source` | `OpsClassifySource` |
| `ops.classify.report` | `OpsClassifyReport` |
| `ops.classify.template` | `OpsClassifyTemplate` |
| `ops.classify.seeds` | `OpsClassifySeeds` |
| `trace.corpus.load` | `TraceCorpusLoad` |
| `trace.corpus.source` | `TraceCorpusSource` |
| `trace.corpus.reachable` | `TraceCorpusReachable / LoadedKernel.Reachable` |
| `trace.corpus.walk_seed` | `TraceCorpusWalkSeed` |
| `trace.corpus.fingerprint` | `TraceCorpusFingerprint` |
| `trace.corpus.compare` | `TraceCorpusCompare` |
| `trace.corpus.compare_inline` | `TraceCorpusCompareInline` |
| `naming.corpus.analyze` | `NamingCorpusAnalyze` |
| `naming.corpus.compare` | `NamingCorpusCompare` |
| `seeds.parse` | `SeedsParse` |
| `seeds.parse_launcher` | `SeedsParseLauncher` |
| `seeds.parse_kernel` | `SeedsParseKernel` |
| `seeds.parse_url` | `SeedsParseURL` |
| `seeds.resolve_url` | `SeedsResolveURL` |
| `seeds.token` | `SeedsToken` |
| `seeds.sites` | `SeedsSites` |
| `seeds.site` | `SeedsSite` |
| `seeds.fetch` | `SeedsFetch` |
| `seeds.fetch_direct` | `SeedsFetchDirect` |
| `seeds.client` | `SeedsClient` |
| `seeds.import` | `SeedsImport` |
| `headers.codec` | `HeadersCodec` |
| `headers.parse` | `HeadersParse` |
| `headers.encode` | `HeadersEncode` |
| `headers.decode` | `HeadersDecode` |
| `headers.encode_bytes` | `HeadersEncodeBytes` |
| `headers.decode_bytes` | `HeadersDecodeBytes` |
| `headers.encode_finalized` | `HeadersEncodeFinalized` |
| `headers.decode_finalized` | `HeadersDecodeFinalized` |
| `payload.codec` | `PayloadCodec` |
| `payload.encode` | `PayloadEncode` |
| `payload.decode` | `PayloadDecode` |
| `payload.encode_bytes` | `PayloadEncodeBytes` |
| `payload.decode_bytes` | `PayloadDecodeBytes` |
| `pack` | `Pack` |
| `unpack` | `Unpack` |
| `unpack_canonical` | `UnpackCanonical` |

Loaded decode operations use `LoadArtifact` or `CompileLoadedArtifact`, then
`Decode`, `Explain`, `Encode`, `EncodeSignals`, `Schema`, and explicit
`LoadedArtifact.JSON`. All also have methods on the loaded artifact. `Close`
waits for in-flight calls. Legacy `Load` and `Generator` remain supported.

Portable projections follow the native contract: classification report/template
reconstruct from raw source; fingerprint replays a thread; naming comparison
reconstructs analyses from source arrays; codec calls pass explicit profiles.
Rust HTTP trait injection is represented by explicit acquisition plus
`SeedsParse`/`SeedsImport`; portable fetch uses bounded native client options.

`Ops.Classify.ReportDocument(report json.RawMessage)` also accepts an explicit
serialized PipelineReport; `Ops.Classify.TemplateDocument(source string,
template json.RawMessage)` accepts a serialized HandlerTemplate plus handler
text. `Report(source)` and `Template(source, handlerIndex)` retain the portable
raw-source reconstruction forms.
