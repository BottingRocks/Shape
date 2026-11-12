"""Operation-oriented acceptance, driven by the provider's versioned registry."""
import copy
import json
from pathlib import Path
import shutil

CONSUMERS = ("node", "python", "go")


def registry(path):
    document = json.loads(Path(path).read_text())
    if document.get("contract_version") != 2:
        raise ValueError(f"expected portable contract version 2: {path}")
    operations = [entry["id"] for entry in document["operations"]]
    if not operations or len(operations) != len(set(operations)):
        raise ValueError(f"operation IDs must be nonempty and unique: {path}")
    if document["cache"]["default"] != {"mode": "disabled"}:
        raise ValueError("named operation cache must default to disabled")
    defaults = document["defaults"]["trace_program"]
    if (defaults["thread_cleanup"], defaults["no_cleanup"], defaults["jobs"]) != (True, False, None):
        raise ValueError("named trace full cleanup defaults changed")
    return document, operations


def exercise(c, capture, root, inventory):
    document, operations = registry(inventory)
    version = c.case("v2-version", "version", {}, scope="v2", languages=("rust", "c"))
    c.expect(version["contract_version"] == document["contract_version"], "provider contract version differs")
    c.expect(set(version["commands"]) == set(operations), "provider catalogue differs from authoritative registry")
    actual = c.case("named-surface", "surface", operations, scope="surface", languages=CONSUMERS)
    c.expect(actual == operations, "named API mapping lost portable operations")

    def named(name, operation, body, **kwargs):
        c.expect(operation in operations, f"test refers to unregistered operation {operation}")
        return c.case(name, operation, body, scope="named", **kwargs)

    named("named-cleanup", "cleanup.thread", {"source": "function f(a) { let $SSA_1 = a; return $SSA_1; }"})
    cards = named("named-catalog", "signals.catalog", {})
    c.expect(bool(cards), "signal catalogue is empty")
    named("named-describe", "signals.describe", {"name": "DeviceMemory"})
    named("named-unknown-signal", "signals.describe", {"name": "ContractUnknownSignal"}, error="invalid_request")

    # Existing public mini-JS: exercise finders and optional naming without VM recovery.
    source = (root / "tests/fixtures/sdk/decode/program.js").read_text()
    session = json.loads((root / "tests/fixtures/sdk/decode/session.json").read_text())
    compiled = named("named-compile", "artifact.compile", {"source": source, "session": session, "site_id": "example"})
    original = c.artifact
    try:
        c.artifact = c.temp / "compiled-artifact.json"
        c.artifact.write_text(json.dumps(compiled))
        source_capture = json.loads((root / "tests/fixtures/sdk/decode/headers.json").read_text())
        compiled_decoded = c.case("loaded-compiled", "decode", {"capture": source_capture, "verify": True}, scope="loaded")
        c.expect(compiled_decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"] == 9,
                 "public program compile/decode did not recover memory=9")
    finally:
        c.artifact = original
    for operation in ("signals.show", "signals.inputs", "signals.contracts", "naming.plan", "naming.inventory"):
        named("named-" + operation.replace(".", "-"), operation, {"source": source})
    named("named-naming-corpus", "naming.corpus.analyze", {"sources": [{"name": "public.js", "source": source}]})
    named("named-naming-compare", "naming.corpus.compare", {"baseline": [], "current": []})
    named("named-trace-compare", "trace.corpus.compare", {"baseline": {}, "current": {}})
    named("named-trace-inline", "trace.corpus.compare_inline", {"outlined": {}, "inline": {}})
    named("named-classify-summary", "ops.classify.seeds", {"seeds": []})

    # This tiny public VM fixture is also the native provider's cleanup-defaults
    # fixture; it does not perform expensive captured-seed recovery.
    raw_source = (root / "tests/fixtures/javascript-vm-prefix.raw.js").read_text()
    classification = named("named-classify", "ops.classify.source", {"source": raw_source})
    named("named-classify-report", "ops.classify.report", {"source": raw_source})
    named("named-classify-template", "ops.classify.template", {
        "source": raw_source, "handler_index": classification["ops"][0]["handler_index"]})
    inherited = c.env
    environment_cache = c.temp / "environment-cache"
    c.env = dict(inherited, SHAPE_KERNEL_CACHE=str(environment_cache))
    try:
        default_trace = named("named-trace-defaults", "trace.program", {"source": raw_source})
        explicit_trace = named("named-trace-full-cleanup", "trace.program", {"source": raw_source,
            "thread_cleanup": True, "no_cleanup": False, "jobs": None, "cache": {"mode": "disabled"}})
        c.expect(default_trace == explicit_trace, "implicit full cleanup defaults differ from explicit Rust policy")
        c.expect(not environment_cache.exists(), "disabled/default policy accessed environment disk cache")
        directory = c.temp / "directory-cache"
        kernel = named("named-cache-directory", "trace.corpus.load", {"source": raw_source,
            "name": "public.js", "cache": {"mode": "directory", "directory": str(directory)}})
        c.expect((directory / "public.js.kernel.json").is_file(), "directory policy did not publish a kernel")
        cached = named("named-cache-reuse", "trace.corpus.load", {"source": raw_source,
            "name": "public.js", "cache": {"mode": "directory", "directory": str(directory)}})
        c.expect(kernel == cached, "directory hit changed kernel artifact")
        shutil.rmtree(directory)
        previous = c.artifact
        try:
            c.artifact = c.temp / "loaded-kernel.json"
            c.artifact.write_text(json.dumps(kernel))
            reused = c.case("loaded-kernel-trace", "trace.program", {"source": raw_source}, scope="kernel")
            # Name differences are meaningful engine inputs. Compare a matching
            # named trace rather than stripping result fields.
            reference = named("named-kernel-reference", "trace.program", {"source": raw_source})
            c.expect(reused == reference, "loaded kernel tracing diverges from raw-source operation")
            c.case("loaded-kernel-identity", "trace.program", {"source": raw_source + "\n"}, scope="kernel", error="failed")
            c.expect(not directory.exists(), "loaded in-memory kernel unexpectedly recreated disk cache")
        finally:
            c.artifact = previous
        directory.mkdir()
        corrupt = directory / "public.js.kernel.json"
        corrupt.write_text("{broken")
        named("named-cache-corrupt", "trace.corpus.load", {"source": raw_source,
            "name": "public.js", "cache": {"mode": "directory", "directory": str(directory)}}, error="failed")
        fresh = named("named-cache-disabled", "trace.corpus.load", {"source": raw_source,
            "name": "public.js", "cache": {"mode": "disabled"}})
        c.expect(fresh == kernel, "disabled policy failed to compile independently of corrupt disk state")
        c.expect(corrupt.read_text() == "{broken", "disabled policy changed an unrelated explicit cache")
        named("named-cache-environment", "trace.corpus.load", {"source": raw_source,
            "name": "public.js", "cache": {"mode": "environment"}})
        entries = list(environment_cache.glob("*.kernel.json"))
        # Legacy environment resolution keys entries by source/executable hash,
        # unlike Directory's explicit name. Inspect the published artifact too.
        c.expect(bool(entries), "explicit environment policy did not use configured cache")
        for entry in entries:
            c.expect(json.loads(entry.read_text()) == kernel, "environment cache published another kernel")
    finally:
        c.env = inherited

    # Offline seed operations; never fetch external URLs or write a seed store.
    named("named-url", "seeds.parse_url", {"input": "https://example.test/seed.js?seed=public123"})
    named("named-resolve", "seeds.resolve_url", {"reference": "../kernel.js", "base": "https://example.test/scripts/init.js"})
    # Acquisition classification deliberately rejects tiny scripts (<4096 B).
    # Use the committed public kernel rather than padding an invented snippet.
    seed_bytes = (root / "tests/fixtures/kernels/javascript/kernel.raw.js").read_bytes()
    named("named-kernel-parse", "seeds.parse_kernel", {"bytes": list(seed_bytes)})
    named("named-client", "seeds.client", {"timeout_ms": 1000, "connect_timeout_ms": 500, "max_body_bytes": 4096, "max_redirects": 0})
    sites = named("named-sites", "seeds.sites", {})
    c.expect(bool(sites), "seed registry is empty")
    named("named-site", "seeds.site", {"name": sites[0]["name"]})

    fixtures = root / "tests/fixtures/sdk/generator"
    value = json.loads((fixtures / "superpack-values.json").read_text())["value"]
    packed = named("named-pack", "pack", {"value": value})
    unpacked = named("named-unpack", "unpack", {"bytes": packed})
    c.expect(unpacked["value"] == value, "typed superpack lost a tagged value or exact IEEE bits")
    canonical = named("named-canonical", "unpack_canonical", {"bytes": packed})
    c.expect(canonical == value, "canonical superpack roundtrip differs")
    named("named-invalid-packed", "unpack", {"bytes": []}, error="failed")

    profile = json.loads((fixtures / "profile.json").read_text())
    report = json.loads((fixtures / "report.json").read_text())
    named("named-header-profile", "headers.codec", {"profile": profile})
    payload_profile = {key: item for key, item in profile.items() if key != "header_name"}
    named("named-payload-profile", "payload.codec", {"profile": payload_profile})
    framed = named("named-payload-encode", "payload.encode", {"profile": payload_profile, "report": report})
    restored = named("named-payload-decode", "payload.decode", {
        "profile": payload_profile, "text": framed, "positional_count": len(report["positionalSignals"]), "keysets": []})
    # Superpack canonically orders object entries. The input fixture includes
    # intentionally unsorted object tuples; assert exact canonical wire reuse.
    reframed = named("named-payload-reencode", "payload.encode", {"profile": payload_profile, "report": restored})
    c.expect(reframed == framed, "canonical report re-encoding changed framed bytes")
    raw = [0, 128, 255, 17]
    framed_raw = named("named-raw-encode", "payload.encode_bytes", {"profile": payload_profile, "bytes": raw, "seeds": [13, 17]})
    payload = named("named-raw-decode", "payload.decode_bytes", {"profile": payload_profile, "text": framed_raw})
    c.expect(payload["data"] == {"kind": "raw", "bytes": raw}, "payload bytes changed or became base64")
    headers = named("named-header-bytes", "headers.encode_bytes", {"profile": profile, "payload": payload})
    named("named-header-decode-bytes", "headers.decode_bytes", {"profile": profile, "headers": headers})

    # One immutable loaded artifact per process, repeated direct calls before close.
    exported = c.case("loaded-export", "artifact", {}, scope="loaded")
    c.expect(exported == json.loads(c.artifact.read_text()), "explicit export changed artifact")
    schema = c.case("loaded-schema", "schema", {}, scope="loaded")
    named_schema = named("named-compact", "artifact.compact_schema", {"artifact": exported})
    c.expect(schema == named_schema, "loaded and document compact schemas differ")
    decoded = c.case("loaded-default", "decode", {"capture": capture}, scope="loaded")
    c.expect(decoded["metadata"]["signals"] == {}, "default decode eagerly expanded metadata")
    c.expect(decoded["metadata"]["exactReencode"] is False, "default decode unexpectedly enabled verification")
    detailed = c.case("loaded-detailed", "decode", {"capture": capture, "metadata": True, "verify": True}, scope="loaded")
    from contract import large_integers
    c.expect(bool(list(large_integers(detailed))), "direct named API did not exercise exact 64-bit metadata")
    c.case("loaded-explain", "explain", {"decoded": decoded, "signal": "DeviceMemory", "verify": True}, scope="loaded")
    unchanged = c.case("loaded-encode", "encode", {"decoded": decoded}, scope="loaded")
    c.expect(unchanged == capture, "loaded unchanged encode lost captured headers")
    edited = copy.deepcopy(decoded)
    edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
    encoded = c.case("loaded-edit", "encode", {"decoded": edited}, scope="loaded")
    again = c.case("loaded-edited-decode", "decode", {"capture": encoded, "verify": True}, scope="loaded")
    c.expect(again["positionalSignals"] == edited["positionalSignals"], "direct edit changed signal values")
    # Fresh encryption is nondeterministic. Call each public encode_signals
    # wrapper, then compare every consumer decoding that producer's capture.
    from contract import LANGUAGES, dump, run
    request = dump(c.temp / "fresh-request.json", {"signals": edited["positionalSignals"]})
    for language in LANGUAGES:
        envelope = json.loads(run([*c.commands[language], c.artifact, "loaded", "encode", request], env=c.env))
        dump(c.temp / f"loaded-fresh-{language}.json", envelope)
        c.checks += 1
        c.expect(envelope.get("ok") is True, f"direct encode_signals/{language}: {envelope}")
        if envelope.get("ok"):
            result = c.case(f"loaded-fresh-from-{language}", "decode", {"capture": envelope["value"]}, scope="loaded")
            c.expect(result["positionalSignals"] == edited["positionalSignals"],
                     f"direct encode_signals/{language} changed named fields")
    wrong = dict(capture, siteId="wrong-site")
    c.case("loaded-wrong-site", "decode", {"capture": wrong}, scope="loaded", error="invalid_request")
