# Native operation contract v2

Implemented by the Rust portable provider and additive C ABI.

Machine-readable authority: operations.json (also version-controlled sdk/operations.json).
Consumers: use shape_call_v2 for named operations. shape_call remains legacy-compatible.
Cache request: omitted/null or {"mode":"disabled"}; {"mode":"directory","directory":"..."}; {"mode":"environment"}. No process environment mutation.
Cleanup defaults: trace.program defaults thread_cleanup=true,no_cleanup=false,jobs=null. Raw artifact.compile uses those same defaults. Full mechanical cleanup includes parallel per-thread cleanup and whole-program parameter/capture normalization and kernel-value substitution. Semantic naming operations are optional. Explicit no_cleanup=true retains diagnostic output that skips the full pipeline. Cache is disabled by default.
Decode handles: shape_artifact_load/call/free are ABI-compatible aliases of shape_generator_load/call/free, with additional artifact export method. Direct shape_decode/shape_encode/shape_explain/shape_schema functions take the same loaded handle and request JSON. Never reload artifacts per operation.
Kernel handles: shape_kernel_load/call/free parse once; operations trace.program, trace.thread, trace.corpus.reachable, kernel.to_json. Reuse independent of cache. trace.program checks raw-source identity.
Bytes are u8 JSON arrays. Integers remain exact JSON tokens including u64; PackValue numbers are 16 hex IEEE bits. Envelopes and all request/result/default/error conventions are in operations.json.

Coverage matrix: each facade/extensions public function maps to the identically named operations.json ID (Rust :: becomes .); decode/encode/explain/schema and artifact.load/from_artifact/to_json map to decode loaded handle methods, kernel.load/to_json also have stateless projections and kernel loaded interfaces. trace.program_with_artifact maps to kernel handle trace.program or trace.program artifact field. encode_signals maps to loaded encode signals input. version/commands map to v2 version result. No operation families omitted.
Technical distinctions: Rust classify.report/template accept serialized PipelineReport/HandlerTemplate unchanged; optional source reconstruction is also supported (template selects handler_index). corpus.fingerprint replays a specified loaded kernel thread instead of accepting PrefixTraceReport. naming.corpus.compare reconstructs analyses from named source arrays. headers/payload codecs use explicit validated profile objects per portable call. seeds.client returns normalized config used by fetch/fetch_direct; trait-based injected HTTP clients remain Rust-specific, equivalent portable custom acquisition passes fetched bytes to seeds.parse/import. Corpus maps use arrays of explicit Function records rather than JSON object tuple keys. These are deliberate ABI projections, not deferred implementations.

Versioning: additive v2 dispatch preserves legacy defaults/symbols. Unknown operation returns unknown_command, malformed requests invalid_request, engine/validation errors failed, caught panics panic. Core artifacts retain independent schema versions. Live handles may be shared across calls; caller must wait for all calls before freeing exactly once. All result strings use shape_free.


## Complete operation coverage matrix

| Rust facade/extension operation | Portable operation or explicit projection |
|---|---|
| `version` | `version` — {version,commands,contract_version} |
| `deob` | `deob` — {code:string,artifact?:KernelArtifact} |
| `report` | `report` — KernelArtifact |
| `kernel::show` | `kernel.show` — KernelProperties |
| `ops::list` | `ops.list` — OpsList |
| `ops::show` | `ops.show` — OpsShow |
| `trace::program` | `trace.program` — {code,complete,traced,stopped:[{function,reason}],kernel_globals} |
| `trace::thread` | `trace.thread` — ThreadTrace |
| `cleanup::thread` | `cleanup.thread` — {code,cleanup} |
| `naming::threads` | `naming.threads` — {source,report} |
| `naming::collectors` | `naming.collectors` — {source,report} |
| `naming::plan` | `naming.plan` — SemanticNamingPlan |
| `naming::inventory` | `naming.inventory` — {function_index,role_graph} |
| `signals::schema` | `signals.schema` — SignalSchemaReport |
| `signals::show` | `signals.show` — ProgramProperties |
| `signals::inputs` | `signals.inputs` — ProgramInputContextReport |
| `signals::contracts` | `signals.contracts` — SignalContractReport |
| `pin` | `pin` — SeedLayout |
| `session::from_initializer` | `session.from_initializer` — Session |
| `artifact::compile` | `artifact.compile` — DecodeArtifact |
| `artifact::compact_schema` | `artifact.compact_schema` — CompactSeedSchema |
| `signals::catalog` | `signals.catalog` — CatalogCard[] |
| `signals::describe` | `signals.describe` — CatalogCard |
| `signals::compact_schema` | `signals.compact_schema` — CompactSeedSchema |
| `kernel::load` | `kernel.load` — KernelArtifact (validated) |
| `kernel::to_json` | `kernel.to_json` — string (validated artifact JSON) |
| `ops::classify::source` | `ops.classify.source` — Classification |
| `ops::classify::report` | `ops.classify.report` — Classification; supplied serialized report or reconstruction from raw source |
| `ops::classify::template` | `ops.classify.template` — ClassifiedOp; source is handler text for explicit template, raw source for handler_index reconstruction |
| `ops::classify::seeds` | `ops.classify.seeds` — [{kind,nature,scope,handlers:[[seed_index,handler_index]]}] |
| `trace::corpus::load` | `trace.corpus.load` — KernelArtifact |
| `trace::corpus::source` | `trace.corpus.source` — [{function:{descriptor,pc,state},print,code,created:[{descriptor,pc,state}]}] |
| `trace::corpus::reachable` | `trace.corpus.reachable` — [{function:{descriptor,pc,state},print,code,created:[{descriptor,pc,state}]}] |
| `trace::corpus::walk_seed` | `trace.corpus.walk_seed` — object function-key -> fingerprint; explicit file read/optional branches output |
| `trace::corpus::fingerprint` | `trace.corpus.fingerprint` — {print,code}; replay thread instead of nonserializable PrefixTraceReport |
| `trace::corpus::compare` | `trace.corpus.compare` — {regressions:string[],changes:string[]} |
| `trace::corpus::compare_inline` | `trace.corpus.compare_inline` — string[] |
| `naming::corpus::analyze` | `naming.corpus.analyze` — [{name,plan,rename:{source,source_sha256,transformed_sha256,applied,skipped,ambiguous,plan}}] |
| `naming::corpus::compare` | `naming.corpus.compare` — [{name,baseline_present,current_present,missing_roles,added_roles,name_divergences}]; plans reconstructed from supplied sources |
| `seeds::parse` | `seeds.parse` — {kind:launcher / kernel,launcher?:{kernel_ref},kernel?:{bytes,sha256,statements,functions}} |
| `seeds::parse_launcher` | `seeds.parse_launcher` — {kernel_ref} |
| `seeds::parse_kernel` | `seeds.parse_kernel` — {bytes,sha256,statements,functions} |
| `seeds::parse_url` | `seeds.parse_url` — string URL |
| `seeds::resolve_url` | `seeds.resolve_url` — string URL |
| `seeds::token` | `seeds.token` — string validated token |
| `seeds::sites` | `seeds.sites` — Site[] |
| `seeds::site` | `seeds.site` — Site |
| `seeds::fetch` | `seeds.fetch` — {seed,launcher_url,kernel_url,launcher:u8[] / null,kernel:u8[],kernel_summary}; explicit network only |
| `seeds::fetch_direct` | `seeds.fetch_direct` — {seed,launcher_url,kernel_url,launcher:u8[] / null,kernel:u8[],kernel_summary}; explicit network only |
| `seeds::client` | `seeds.client` — ClientConfig normalized; portable requests pass config, Rust HttpClient injection remains Rust-only |
| `seeds::import` | `seeds.import` — {status,kernel_path,launcher_path,launcher_differs}; explicit persistence |
| `headers::codec` | `headers.codec` — Profile validated; portable calls supply immutable profile |
| `headers::parse` | `headers.parse` — map header name -> string |
| `headers::encode` | `headers.encode` — map header name -> string |
| `headers::decode` | `headers.decode` — HeaderReport |
| `headers::encode_bytes` | `headers.encode_bytes` — map header name -> string |
| `headers::decode_bytes` | `headers.decode_bytes` — Payload |
| `headers::encode_finalized` | `headers.encode_finalized` — map header name -> string |
| `headers::decode_finalized` | `headers.decode_finalized` — {text,context} |
| `payload::codec` | `payload.codec` — PayloadProfile validated |
| `payload::encode` | `payload.encode` — string |
| `payload::decode` | `payload.decode` — Report |
| `payload::encode_bytes` | `payload.encode_bytes` — string |
| `payload::decode_bytes` | `payload.decode_bytes` — Payload |
| `pack` | `pack` — u8[] |
| `unpack` | `unpack` — {value:PackValue,original_bytes:u8[],canonical:bool} |
| `unpack_canonical` | `unpack_canonical` — PackValue |
| `artifact::load`, `artifact::from_artifact`, `LoadedArtifact::artifact/to_json` | `shape_artifact_load`, loaded `artifact` export; validated once |
| `decode`, `encode`, `encode_signals`, `explain`, `schema`, `Generator::*` | `shape_artifact_call` methods plus direct C functions; legacy generator preserved |
| `trace::program_with_artifact` | `shape_kernel_call(handle, trace.program, request)` or v2 artifact document |
| `commands` | v2 version.commands; legacy commands unchanged |


Directory caches reject mismatched source identity with a failed error. They do not silently replace another seed. Environment caches retain the core best-effort rebuild behavior. Classification nature values are javascript, vm, fused, unrecognized; scope values are common, javascript-vm-only, webassembly-vm-only. The raw-source README program is compiled but requires caller-supplied matching inputs for an end-to-end run.

Compile-handle producer (signature published before implementation):
`ShapeArtifact *shape_artifact_compile(const char *request_json, char **error_out);`
Same request as v2 artifact.compile; returns an owned validated handle without document serialization/reload/revalidation. NULL request means {}; invalid UTF-8/JSON/missing fields produce invalid_request. Success clears *error_out; failure returns NULL and optionally stores owned error envelope (shape_free). Previous error_out value remains caller-owned. Handle lifecycle/aliases/concurrency match shape_artifact_load. Document-returning artifact.compile and explicit artifact export remain supported. Implemented and verified by focused provider and C ABI tests.
