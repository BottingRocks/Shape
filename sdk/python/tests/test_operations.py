"""v2 semantics and native public-fixture coverage; run with uv unittest."""

import contextlib
import io
import hashlib
import copy
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import shape
from shape import _native

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "tests/fixtures"
PUBLIC = ROOT / "sdk/c/tests/fixtures"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


class OperationUnitTests(unittest.TestCase):
    def test_cache_modes_cleanup_defaults_and_environment_untouched(self):
        before = dict(os.environ)
        policies = [
            (None, {"mode": "disabled"}),
            (shape.CachePolicy.disabled(), {"mode": "disabled"}),
            (
                shape.CachePolicy.directory(Path("cache")),
                {"mode": "directory", "directory": "cache"},
            ),
            (shape.CachePolicy.environment(), {"mode": "environment"}),
        ]
        with patch("shape.trace._call", return_value={}) as native:
            for policy, expected in policies:
                shape.trace.program("raw", cache=policy)
                request = json.loads(_native.encode_request(native.call_args.args[1]))
                self.assertEqual(request["cache"], expected)
                self.assertTrue(request["thread_cleanup"])
                self.assertFalse(request["no_cleanup"])
                self.assertNotIn("jobs", request)  # native default is None/auto
                self.assertNotIn("report_dir", request)
            shape.trace.program("raw", thread_cleanup=False, no_cleanup=True, jobs=1)
            request = native.call_args.args[1]
            self.assertFalse(request["thread_cleanup"])
            self.assertTrue(request["no_cleanup"])
            self.assertEqual(request["jobs"], 1)
            shape.trace.program("raw", report_dir="old-cache")
            self.assertEqual(
                native.call_args.args[1]["cache"],
                {"mode": "directory", "directory": "old-cache"},
            )
            with self.assertRaises(shape.ShapeError):
                shape.trace.program(
                    "raw", cache=shape.CachePolicy.disabled(), report_dir="cache"
                )
        self.assertEqual(dict(os.environ), before)

    def test_invalid_cache_fails_without_native(self):
        with patch("shape.trace._call") as native:
            for policy in ({"mode": "environment"}, False, "cache"):
                with self.assertRaises(shape.ShapeError):
                    shape.trace.program("raw", cache=policy)
            native.assert_not_called()
        for args in (
            ("invented",),
            ("directory",),
            ("directory", ""),
            ("disabled", "cache"),
        ):
            with self.assertRaises(shape.ShapeError):
                shape.CachePolicy(*args)

    def test_kernel_document_trace_cleanup_default_and_opt_out(self):
        with patch("shape.trace._call", return_value={}) as native:
            for options, no_cleanup in (({}, False), ({"no_cleanup": True}, True)):
                shape.trace.program_with_artifact("raw", {}, **options)
                request = native.call_args.args[1]
                self.assertTrue(request["thread_cleanup"])
                self.assertEqual(request["no_cleanup"], no_cleanup)
                self.assertIsNone(request["jobs"])

    def test_bytes_and_number_bits_do_not_become_floats(self):
        with patch("shape.payload.operation", return_value="encoded") as native:
            shape.payload.encode_bytes({}, memoryview(b"\x00\x80\xff"), [2**32 - 1, 0])
            request = json.loads(_native.encode_request(native.call_args.args[1]))
            self.assertEqual(request["bytes"], [0, 128, 255])
            self.assertEqual(request["seeds"], [2**32 - 1, 0])
            with self.assertRaises(shape.ShapeError):
                shape.payload.encode_bytes({}, [256], [1, 2])
        number = {"kind": "number", "value": "8000000000000000"}
        with patch("shape.superpack.operation", return_value=[0]) as native:
            self.assertEqual(shape.pack(number), b"\0")
            self.assertEqual(native.call_args.args[1]["value"], number)

    def test_named_dispatch_requires_v2_no_legacy_fallback(self):
        class Legacy:
            def shape_call(self, *args):
                raise AssertionError("must not fall back to legacy")

        with patch("shape._native.lib", return_value=Legacy()):
            with self.assertRaises(shape.ShapeError) as caught:
                shape.signals.catalog()
            self.assertEqual(caught.exception.code, "unsupported_library")

    def test_readme_blocks_compile(self):
        blocks = re.findall(
            r"```python\n(.*?)```", (ROOT / "sdk/python/README.md").read_text(), re.S
        )
        self.assertGreaterEqual(len(blocks), 8)
        for index, block in enumerate(blocks):
            with self.subTest(block=index):
                compile(block, "README:python:{}".format(index), "exec")

    def test_closed_loaded_handles_reject_calls(self):
        for cls in (shape.LoadedArtifact, shape.kernel.LoadedKernel):
            with self.subTest(cls=cls), patch("shape._generator.lib") as native:
                loaded = cls(123)
                loaded.close()
                loaded.close()
                with self.assertRaises(shape.ShapeError):
                    loaded.to_json()
                getattr(
                    native.return_value, "shape_" + loaded._family + "_free"
                ).assert_called_once_with(123)

    def test_direct_operation_failure_releases_active_reference(self):
        loaded = shape.LoadedArtifact(123)
        try:
            with patch(
                "shape._generator.symbol",
                side_effect=shape.ShapeError("failed", "boom"),
            ):
                with self.assertRaises(shape.ShapeError):
                    shape.decode(loaded, {})
            self.assertEqual(loaded._active, 0)
        finally:
            with patch("shape._generator.lib"):
                loaded.close()

    def test_compile_returns_one_loaded_handle(self):
        with (
            patch("shape.artifact.symbol") as symbol,
            patch.object(
                shape.LoadedArtifact, "_from_native", return_value="loaded"
            ) as adopt,
        ):
            symbol.return_value.return_value = 456
            with patch("shape.artifact.load") as load:
                self.assertEqual(
                    shape.artifact.compile("program", {}, "example"), "loaded"
                )
                load.assert_not_called()
            symbol.assert_called_once_with("shape_artifact_compile")
            fields = json.loads(symbol.return_value.call_args.args[0])
            self.assertEqual(fields["cache"], {"mode": "disabled"})
            self.assertFalse(fields["direct"])
            self.assertEqual(fields["input_kind"], "program")
            self.assertEqual(adopt.call_args.args[0], 456)

    def test_native_producer_null_errors_are_not_success(self):
        import ctypes

        with self.assertRaises(shape.ShapeError) as caught:
            shape.LoadedArtifact._from_native(
                None, ctypes.c_void_p(), "shape_artifact_compile"
            )
        self.assertEqual(caught.exception.code, "failed")
        with patch(
            "shape._generator.take",
            return_value=b'{"ok":false,"error":{"code":"invalid_request","message":"bad source"}}',
        ) as take:
            with self.assertRaises(shape.ShapeError) as caught:
                shape.LoadedArtifact._from_native(
                    None, ctypes.c_void_p(123), "shape_artifact_compile"
                )
            self.assertEqual(caught.exception.code, "invalid_request")
            take.assert_called_once_with(123)

    def test_classification_alternative_inputs_are_explicit(self):
        with patch("shape._classify.operation", return_value={}) as native:
            shape.ops.classify.report(report=b"{}")
            self.assertEqual(native.call_args.args[1]["report"], "{}")
            shape.ops.classify.template("handler", template={"handler_index": 9})
            self.assertEqual(native.call_args.args[1]["template"], {"handler_index": 9})
            for kwargs in ({}, {"source": "raw", "report": {}}):
                with self.assertRaises(shape.ShapeError):
                    shape.ops.classify.report(**kwargs)
            for kwargs in ({}, {"handler_index": 0, "template": {}}):
                with self.assertRaises(shape.ShapeError):
                    shape.ops.classify.template("source", **kwargs)

    def test_operation_mapping_resolves_every_named_api(self):
        mapping = read_json(Path(__file__).with_name("operation_coverage.json"))
        for name in mapping.values():
            value = shape
            for part in name.removeprefix("shape.").split("."):
                value = getattr(value, part)
            self.assertTrue(callable(value), name)


class NativeOperationTests(unittest.TestCase):
    """New v2 operation families must run with an explicitly pinned library.

    No skip: absent/stale native code is a failure for this runtime suite.
    """

    @classmethod
    def setUpClass(cls):
        if not os.environ.get("SHAPE_LIBRARY"):
            raise RuntimeError(
                "set SHAPE_LIBRARY to provider native-library.txt path for v2 runtime checks"
            )
        cls.info = shape.version_info()
        if cls.info["contract_version"] != 2:
            raise RuntimeError("native operation contract is not v2")
        cls.raw = (FIXTURES / "javascript-vm-prefix.raw.js").read_text()
        cls.program = (FIXTURES / "sdk/decode/program.js").read_text()
        cls.session = read_json(FIXTURES / "sdk/decode/session.json")
        cls.profile = read_json(FIXTURES / "sdk/generator/profile.json")
        cls.report = read_json(FIXTURES / "sdk/generator/report.json")
        cls.kernel_document = shape.report(cls.raw)

    def test_loaded_decode_edit_export_precision_and_concurrency(self):
        native = _native.lib()
        capture = read_json(PUBLIC / "capture.json")
        with patch.object(
            native, "shape_artifact_load", wraps=native.shape_artifact_load
        ) as loader:
            with shape.artifact.load(PUBLIC / "artifact.json") as loaded:
                document = loaded.artifact()
                exported = loaded.to_json()
                self.assertEqual(
                    json.loads(exported), read_json(PUBLIC / "artifact.json")
                )
                document.clear()  # exported snapshot cannot mutate the handle
                decoded = shape.decode(loaded, capture, verify=True)
                self.assertEqual(
                    decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"], 32
                )
                self.assertEqual(shape.encode(loaded, decoded), capture)
                self.assertEqual(
                    shape.explain(loaded, decoded, "DeviceMemory")["status"], "decoded"
                )
                self.assertEqual(
                    shape.schema(loaded), shape.artifact.compact_schema(exported)
                )
                edited = copy.deepcopy(decoded)
                edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
                encoded = shape.encode(loaded, edited)
                verified = shape.decode(loaded, encoded, verify=True)
                self.assertEqual(
                    verified["positionalSignals"]["DeviceMemory"]["deviceMemory"], 8
                )
                self.assertTrue(verified["metadata"]["exactReencode"])
                bare = shape.encode_signals(loaded, edited["positionalSignals"])
                self.assertEqual(
                    shape.decode(loaded, bare)["positionalSignals"],
                    edited["positionalSignals"],
                )
                errors = []

                def work():
                    try:
                        self.assertEqual(
                            shape.decode(loaded, capture)["positionalSignals"][
                                "DeviceMemory"
                            ]["deviceMemory"],
                            32,
                        )
                    except Exception as error:
                        errors.append(error)

                threads = [threading.Thread(target=work) for _ in range(3)]
                for thread in threads:
                    thread.start()
                for thread in threads:
                    thread.join(10)
                    self.assertFalse(thread.is_alive())
                self.assertEqual(errors, [])
                loader.assert_called_once()
        with shape.artifact.from_artifact(json.loads(exported)) as restored:
            self.assertEqual(
                shape.encode(restored, shape.decode(restored, capture)), capture
            )
            compact = shape.schema(restored)
        with self.assertRaises(shape.ShapeError):
            shape.artifact.load(compact)

    def test_compile_and_source_signal_operations(self):
        native = _native.lib()
        with (
            patch.object(
                native, "shape_artifact_load", wraps=native.shape_artifact_load
            ) as loader,
            patch.object(
                native, "shape_artifact_compile", wraps=native.shape_artifact_compile
            ) as compiler,
        ):
            with shape.artifact.compile(
                self.program, self.session, "example"
            ) as loaded:
                captured = read_json(FIXTURES / "sdk/decode/headers.json")
                decoded = shape.decode(loaded, captured, verify=True)
                self.assertTrue(decoded["metadata"]["exactReencode"])
                self.assertEqual(
                    shape.schema(loaded)["signals"],
                    shape.signals.compact_schema(self.program)["signals"],
                )
            compiler.assert_called_once()
            loader.assert_not_called()  # compile owns validated state directly
        with self.assertRaises(shape.ShapeError) as caught:
            shape.artifact.compile(self.program, {}, "example")
        self.assertEqual(caught.exception.code, "invalid_request")
        self.assertEqual(
            set(shape.operation_commands()),
            set(read_json(Path(__file__).with_name("operation_coverage.json"))),
        )
        self.assertTrue(shape.signals.catalog())
        self.assertTrue(shape.signals.describe("DeviceMemory"))
        for function in (
            shape.signals.schema,
            shape.signals.show,
            shape.signals.inputs,
            shape.signals.contracts,
            shape.naming.threads,
            shape.naming.collectors,
            shape.naming.plan,
            shape.naming.inventory,
        ):
            self.assertIsInstance(function(self.program), dict)
        initializer = (FIXTURES / "generator/seed-pinned-init.js").read_text()
        self.assertEqual(
            shape.session.from_initializer(initializer),
            shape.session_from_initializer(initializer),
        )
        self.assertIsInstance(shape.pin(self.program, session=self.session), dict)
        self.assertTrue(shape.deob(self.raw)["code"])
        self.assertTrue(shape.kernel.show(self.raw))
        self.assertTrue(shape.ops.list(self.raw))

    def test_kernel_reuse_cache_and_corpus(self):
        before = dict(os.environ)
        with tempfile.TemporaryDirectory() as tmp:
            cache = Path(tmp) / "kernel-cache"
            traced = shape.trace.program(self.raw, steps=16, name="public.js")
            self.assertFalse(cache.exists())
            cached = shape.trace.program(
                self.raw,
                steps=16,
                name="public.js",
                cache=shape.CachePolicy.directory(cache),
            )
            self.assertEqual(traced, cached)
            self.assertTrue(cache.exists())
            again = shape.trace.program(
                self.raw,
                steps=16,
                name="public.js",
                cache=shape.CachePolicy.directory(cache),
            )
            self.assertEqual(cached, again)
            shape.trace.program(
                self.raw, steps=16, cache=shape.CachePolicy.environment()
            )
        self.assertEqual(dict(os.environ), before)
        native = _native.lib()
        with patch.object(
            native, "shape_kernel_load", wraps=native.shape_kernel_load
        ) as loader:
            with shape.kernel.load(self.kernel_document) as kernel:
                self.assertEqual(json.loads(kernel.to_json()), self.kernel_document)
                self.assertEqual(
                    shape.trace.program_with_artifact(self.raw, kernel, steps=16),
                    traced,
                )
                walked = shape.trace.corpus.reachable(kernel, steps=16)
                self.assertTrue(walked)
                function = walked[0]["function"]
                options = {
                    "pc": function["pc"],
                    "state": function["state"],
                    "descriptor": function["descriptor"],
                    "steps": 16,
                }
                self.assertEqual(
                    shape.trace.thread(artifact=kernel, **options),
                    shape.trace.thread(artifact=self.kernel_document, **options),
                )
                loader.assert_called_once()
                with self.assertRaises(shape.ShapeError):
                    shape.trace.program_with_artifact(
                        self.raw + "\n// wrong", kernel, steps=16
                    )
        self.assertEqual(
            json.loads(shape.kernel.to_json(self.kernel_document)), self.kernel_document
        )
        self.assertEqual(shape.trace.corpus.load(self.raw), self.kernel_document)
        self.assertTrue(shape.trace.corpus.source(self.raw, steps=16))
        self.assertTrue(shape.trace.corpus.reachable(self.kernel_document, steps=16))
        self.assertIn(
            "print", shape.trace.corpus.fingerprint(self.kernel_document, **options)
        )
        full = shape.trace.corpus.walk_seed(
            FIXTURES / "javascript-vm-prefix.raw.js",
            steps=16,
            self_check=True,
            cleanup=True,
        )
        self.assertTrue(full)
        self.assertEqual(
            shape.trace.corpus.compare({"public": full}, {"public": full})[
                "regressions"
            ],
            [],
        )
        self.assertEqual(shape.trace.corpus.compare_inline({}, {}), [])

    def test_classification_and_naming_corpus(self):
        classified = shape.ops.classify.source(self.raw)
        self.assertEqual(shape.ops.classify.report(self.raw), classified)
        self.assertTrue(classified["ops"])
        index = classified["ops"][0]["handler_index"]
        self.assertEqual(
            shape.ops.classify.template(self.raw, index)["handler_index"], index
        )
        self.assertTrue(shape.ops.classify.seeds([classified]))
        template = {
            "handler_index": 9,
            "table_key": 9,
            "semantic_identity": "synthetic-return",
            "source_span": {"start": 0, "end": 9},
            "status": "Executable",
            "steps": [
                {
                    "order": 0,
                    "span": {"start": 0, "end": 9},
                    "operation": {"Return": {"value": {"Literal": {"Number": "1"}}}},
                }
            ],
            "unsupported": [],
        }
        explicit = shape.ops.classify.template("return 1;", template=template)
        self.assertEqual(explicit["handler_index"], 9)
        self.assertEqual(explicit["kind"], "return")
        with self.assertRaises(shape.ShapeError):
            shape.ops.classify.report(report={})
        shape.ops.show(self.raw, index)
        sources = [
            {
                "name": "public",
                "source": "(function main(){ var registry=[1,2,3]; function entry(){return registry[0];} return {init:entry}; })();",
            }
        ]
        analyzed = shape.naming.corpus.analyze(sources)
        self.assertEqual(analyzed[0]["name"], "public")
        same = shape.naming.corpus.compare(sources, sources)[0]
        self.assertEqual(same["missing_roles"], [])
        self.assertEqual(same["name_divergences"], [])

    def test_header_payload_and_pack_known_answers(self):
        profile = shape.headers.codec(self.profile)
        payload_profile = shape.payload.codec(
            {key: self.profile[key] for key in ("format", "cipher", "alphabet", "keys")}
        )
        report = self.report
        count, keysets = len(report["positionalSignals"]), report["keysets"]
        text = shape.payload.encode(payload_profile, report)
        recovered = shape.payload.decode(payload_profile, text, count, keysets)
        self.assertEqual(shape.payload.encode(payload_profile, recovered), text)
        self.assertEqual(recovered["seeds"], report["seeds"])
        headers = shape.headers.encode(profile, report)
        self.assertEqual(
            shape.headers.parse(
                "\n".join("{}: {}".format(k, v) for k, v in headers.items())
            ),
            headers,
        )
        decoded_headers = shape.headers.decode(profile, headers, count, keysets)
        self.assertEqual(shape.headers.encode(profile, decoded_headers), headers)
        finalized = shape.headers.decode_finalized(profile, headers)
        self.assertEqual(
            shape.headers.encode_finalized(
                profile, finalized["text"], finalized["context"]
            ),
            headers,
        )
        raw = read_json(FIXTURES / "sdk/generator/raw.json")
        framed = shape.headers.encode_bytes(profile, raw)
        decoded_raw = shape.headers.decode_bytes(profile, framed)
        self.assertEqual(decoded_raw["seeds"], raw["seeds"])
        self.assertEqual(decoded_raw["data"], raw["data"])
        self.assertEqual(shape.headers.encode_bytes(profile, decoded_raw), framed)
        for answer in read_json(FIXTURES / "generator/known-answers.json"):
            plain = bytes.fromhex(answer["plaintext_hex"])
            self.assertEqual(
                shape.payload.encode_bytes(
                    {
                        key: answer["profile"][key]
                        for key in ("format", "cipher", "alphabet", "keys")
                    },
                    plain,
                    answer["seeds"],
                ),
                answer["value"],
            )
            self.assertEqual(
                bytes(
                    shape.payload.decode_bytes(
                        {
                            key: answer["profile"][key]
                            for key in ("format", "cipher", "alphabet", "keys")
                        },
                        answer["value"],
                    )["data"]["bytes"]
                ),
                plain,
            )
        for answer in read_json(FIXTURES / "generator/superpack-known-answers.json"):
            request = answer["request"]
            packed = shape.pack(request["value"], request.get("options"))
            self.assertEqual(packed.hex(), answer["hex"])
            if answer["decodable"]:
                decoded = shape.unpack(packed, request.get("options"))
                self.assertTrue(decoded["canonical"])
                self.assertEqual(bytes(decoded["original_bytes"]), packed)
                self.assertEqual(
                    shape.unpack_canonical(packed, request.get("options")),
                    decoded["value"],
                )
            else:
                with self.assertRaises(shape.ShapeError):
                    shape.unpack(packed, request.get("options"))

    def test_seed_operations_without_network(self):
        # These are the public Rust SDK test structures, not captured scripts.
        launcher = b'var s=document.createElement("script");s.src="/kernel.js?seed=public";document.head.appendChild(s);'
        self.assertEqual(
            shape.seeds.parse_launcher(launcher)["kernel_ref"], "/kernel.js?seed=public"
        )
        self.assertEqual(shape.seeds.parse(launcher)["kind"], "launcher")
        kernel = (FIXTURES / "kernels/javascript/kernel.raw.js").read_bytes()
        self.assertEqual(shape.seeds.parse(kernel)["kind"], "kernel")
        self.assertEqual(shape.seeds.parse_kernel(kernel)["bytes"], len(kernel))
        url = "https://example.test/kernel.js?seed=public"
        self.assertEqual(shape.seeds.parse_url(url), url)
        self.assertEqual(
            shape.seeds.resolve_url(
                "/kernel.js?seed=public", "https://example.test/start"
            ),
            url,
        )
        self.assertEqual(shape.seeds.token(url), "public")
        sites = shape.seeds.sites()
        self.assertTrue(sites)
        self.assertEqual(shape.seeds.site(sites[0]["name"]), sites[0])
        self.assertEqual(shape.seeds.client(timeout_ms=1000)["timeout_ms"], 1000)
        with tempfile.TemporaryDirectory() as tmp:
            stored = shape.seeds.import_seed(tmp, "public", kernel, launcher=launcher)
            self.assertEqual(Path(stored["kernel_path"]).read_bytes(), kernel)
            self.assertEqual(Path(stored["launcher_path"]).read_bytes(), launcher)
        # Successful acquisition uses loopback only, no external HTTP.
        from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                body = launcher if self.path.startswith("/launcher") else kernel
                self.send_response(200)
                self.send_header("Content-Type", "application/javascript")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            base = "http://127.0.0.1:{}".format(server.server_port)
            config = shape.seeds.client(
                timeout_ms=5000,
                connect_timeout_ms=1000,
                max_body_bytes=len(kernel) + 1024,
                max_redirects=2,
            )
            for fetch, url in (
                (shape.seeds.fetch, base + "/launcher.js"),
                (shape.seeds.fetch_direct, base + "/kernel.js?seed=public"),
            ):
                fetched = fetch(url, client=config)
                normalized = bytes(fetched["kernel"])
                self.assertTrue(normalized)
                self.assertTrue(kernel.startswith(normalized))
                self.assertEqual(
                    shape.seeds.parse_kernel(normalized), fetched["kernel_summary"]
                )
                expected_seed = (
                    "public"
                    if fetch is shape.seeds.fetch
                    else hashlib.sha256(normalized).hexdigest()
                )
                self.assertEqual(fetched["seed"], expected_seed)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(5)
        # Invalid URL is rejected before any HTTP.
        for function in (shape.seeds.fetch, shape.seeds.fetch_direct):
            with self.assertRaises(shape.ShapeError):
                function("file:///not-http")

    def test_errors_validation_precision(self):
        for function, args in (
            (shape.artifact.load, (b"{}",)),
            (shape.kernel.load, (b"{}",)),
            (shape.signals.describe, ("not-a-signal",)),
            (shape.headers.codec, ({"format": "invalid"},)),
            (shape.payload.decode_bytes, (self.profile, "!")),
            (shape.trace.corpus.compare, ([], {})),
            (shape.naming.corpus.analyze, ([{"name": "", "source": ""}],)),
        ):
            with self.subTest(function=function), self.assertRaises(shape.ShapeError):
                function(*args)
        # Full u64 resource limits cross native JSON without float rounding.
        for integer in (2**53 + 1, 2**64 - 1):
            normalized = shape.seeds.client(max_body_bytes=integer)
            self.assertEqual(normalized["max_body_bytes"], integer)
            self.assertIs(type(normalized["max_body_bytes"]), int)
        # Cipher counters are deliberately bounded below 2^53, unlike limits.
        profile = {
            key: copy.deepcopy(self.profile[key])
            for key in ("format", "cipher", "alphabet", "keys")
        }
        profile["cipher"]["counter"] = 2**53 + 1
        with self.assertRaises(shape.ShapeError):
            shape.payload.codec(profile)
        # Exact 64-bit IEEE patterns survive the typed value boundary.
        for bits in ("4340000000000001", "bff0000000000000", "8000000000000000"):
            value = {"kind": "number", "value": bits}
            self.assertEqual(shape.unpack_canonical(shape.pack(value)), value)
        # Native dates are bounded to 48 bits; do not silently truncate u64s.
        with self.assertRaises(shape.ShapeError):
            shape.pack({"kind": "date", "value": 2**64 - 1})

    def test_readme_fixture_recipes(self):
        blocks = re.findall(
            r"```python\n(.*?)```", (ROOT / "sdk/python/README.md").read_text(), re.S
        )
        # All other recipes require explicitly declared user capture files.
        fixture_blocks = [
            block
            for block in blocks
            if 'fixtures = Path("sdk/c/tests/fixtures")' in block
            or 'fixtures = Path("tests/fixtures/sdk/generator")' in block
            or 'seed_path = Path("tests/fixtures/javascript-vm-prefix.raw.js")' in block
            or 'value = {"kind": "array"' in block
        ]
        self.assertEqual(len(fixture_blocks), 4)
        for block in fixture_blocks:
            with self.subTest(first_line=block.splitlines()[0]):
                with contextlib.redirect_stdout(io.StringIO()):
                    exec(compile(block, "README:fixture", "exec"), {})
