"""Tests of the shape Python SDK.

Run: python3 -m unittest discover sdk/python/tests

The native library is found as documented (SHAPE_LIBRARY, shape/lib/,
target/release, target/iter). Generator tests are opt-in: set
SHAPE_TEST_ARTIFACT (a decode artifact JSON file) and SHAPE_TEST_HEADERS (a
captured-headers JSON file decoded by it).
"""

import json
import os
import sys
import threading
import unittest
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import shape  # noqa: E402
from shape._native import encode_request, library_path, parsed_document, text_document, unwrap  # noqa: E402

THREAD = "function f(a) { let $SSA_1 = a; return $SSA_1; }"

# 2^53 + 1: the smallest positive integer a double cannot represent.
BIG = 2**53 + 1


def unsafe_integers(value, path=""):
    """Yield ``(path, int)`` for every integer whose magnitude exceeds 2^53."""
    if isinstance(value, dict):
        for key, item in value.items():
            yield from unsafe_integers(item, "{}.{}".format(path, key))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from unsafe_integers(item, "{}[{}]".format(path, index))
    elif isinstance(value, int) and not isinstance(value, bool):
        if abs(value) > 2**53:
            yield path, value


class JsonBoundaryTests(unittest.TestCase):
    """The binding's JSON text boundary keeps 64-bit integers exact.

    Python ``int`` is arbitrary precision and the binding never converts JSON
    integers to ``float``: requests are dumped from ``int`` values and
    responses are parsed with ``json.loads``. These tests pin that down without
    depending on the native library or a particular artifact.
    """

    def test_compile_direct_option_forwarding_and_default(self):
        for options, expected in [({}, False), ({"direct": False}, False), ({"direct": True}, True)]:
            with self.subTest(options=options), patch("shape._call", return_value={}) as call:
                shape.compile_artifact("program", {}, "example.test", **options)
                command, fields = call.call_args.args
                self.assertEqual(command, "artifact.compile")
                self.assertIs(json.loads(encode_request(fields))["direct"], expected)

    def test_compile_direct_rejects_nonboolean_before_native_call(self):
        for direct in (None, 0, 1, "true", [], {}):
            with self.subTest(direct=direct), patch("shape._call") as call:
                with self.assertRaisesRegex(TypeError, "direct"):
                    shape.compile_artifact("program", {}, "example.test", direct=direct)
                with self.assertRaisesRegex(TypeError, "direct"):
                    shape.call("artifact.compile", direct=direct)
                call.assert_not_called()

    def test_request_preserves_large_integers(self):
        document = {
            "decoded": {
                "positionalSignals": {
                    "Probe": {
                        "numberBits": BIG,
                        "minInt64": -(2**63),
                        "maxInt64": 2**63 - 1,
                        "maxUint64": 2**64 - 1,
                    }
                }
            }
        }
        text = encode_request(document)
        # Exact decimal digits, never a float/exponent form.
        for digits in (b"9007199254740993", b"-9223372036854775808", b"18446744073709551615"):
            self.assertIn(digits, text)
        rebuilt = json.loads(text)
        probe = rebuilt["decoded"]["positionalSignals"]["Probe"]
        self.assertEqual(probe["numberBits"], BIG)
        self.assertEqual(probe["minInt64"], -(2**63))
        self.assertEqual(probe["maxUint64"], 2**64 - 1)
        for name, value in probe.items():
            self.assertIs(type(value), int, name)

    def test_response_preserves_large_integers(self):
        envelope = json.dumps(
            {
                "ok": True,
                "value": {
                    "numberBits": BIG,
                    "unsigned": 2**64 - 1,
                    "signed": -(2**63),
                },
            }
        ).encode("utf-8")
        value = unwrap(envelope)
        self.assertEqual(value["numberBits"], BIG)
        self.assertEqual(value["unsigned"], 2**64 - 1)
        self.assertEqual(value["signed"], -(2**63))
        for name, item in value.items():
            self.assertIs(type(item), int, name)

    def test_parsed_document_preserves_large_integers(self):
        value = parsed_document(json.dumps({"numberBits": BIG}), "decoded")
        self.assertEqual(value["numberBits"], BIG)
        self.assertIs(type(value["numberBits"]), int)

    def test_artifact_document_keeps_large_literals(self):
        # Artifacts carry f64 bit patterns (e.g. schema.wire…literal) above
        # 2^53. A parsed artifact is re-serialized for Generator.load; its
        # integers must survive that trip unchanged. json.dumps/json.loads is
        # the whole path, so this needs no native library.
        text = json.dumps({"schema": {"wire": {"slots": [{"literal": BIG, "unsigned": 2**64 - 1}]}}})
        artifact = json.loads(text)
        self.assertEqual(artifact["schema"]["wire"]["slots"][0]["literal"], BIG)
        rebuilt = json.loads(encode_request({"artifact": artifact}))
        slot = rebuilt["artifact"]["schema"]["wire"]["slots"][0]
        self.assertEqual(slot["literal"], BIG)
        self.assertEqual(slot["unsigned"], 2**64 - 1)

    def test_invalid_requests_raise_shape_error_before_loading(self):
        with patch("shape._native.lib") as native:
            for value in (float("nan"), float("inf"), object(), "\ud800"):
                with self.subTest(value=repr(value)), self.assertRaises(shape.ShapeError) as caught:
                    shape.call("version", bad=value)
                self.assertEqual(caught.exception.code, "invalid_request")
            native.assert_not_called()
        for artifact in ({"bad": object()}, {"bad": float("nan")}, "\ud800"):
            with self.assertRaises(shape.ShapeError) as caught:
                shape.Generator.load(artifact)
            self.assertEqual(caught.exception.code, "invalid_request")

    def test_invalid_document_bytes(self):
        with self.assertRaises(shape.ShapeError) as caught:
            text_document(b"\xff", "capture")
        self.assertEqual(caught.exception.code, "invalid_request")

    def test_none_fields_are_only_omitted_at_top_level(self):
        self.assertEqual(json.loads(encode_request({"session": None, "signals": {"x": None}})),
                         {"signals": {"x": None}})

    def test_malformed_envelopes_raise_shape_error(self):
        for envelope in (b"[]", b"null", b'{"ok":true}', b'{"ok":false}',
                         b'{"ok":false,"error":"oops"}', b'{"ok":1,"value":42}'):
            with self.subTest(envelope=envelope), self.assertRaises(shape.ShapeError) as caught:
                unwrap(envelope)
            self.assertEqual(caught.exception.code, "failed")

    def test_relative_library_override_resolves_to_absolute_path(self):
        relative = os.path.relpath(__file__)
        with patch.dict(os.environ, {"SHAPE_LIBRARY": relative}):
            self.assertEqual(library_path(), Path(__file__).resolve())

    def test_thread_document_options_accept_bytes(self):
        with patch("shape.trace._call") as call:
            shape.trace.thread(artifact=b"{}", pc=0, state=0,
                               initial_slots=b'{"0":9007199254740993}', inputs=b"[]")
            request = json.loads(encode_request(call.call_args.args[1]))
        self.assertEqual(request["artifact"], "{}")
        self.assertEqual(request["initial_slots"], '{"0":9007199254740993}')
        self.assertEqual(request["inputs"], "[]")


class GeneratorLifetimeTests(unittest.TestCase):
    def test_close_rejects_new_calls_and_waits_for_active_call(self):
        # Hold one call in flight without relying on native timing. The closer
        # announces its wait through the condition, then must reject new work.
        generator = shape.Generator(123)
        generator._active = 1
        waiting = threading.Event()
        finished = threading.Event()
        original_wait = generator._cond.wait

        def wait(*args):
            waiting.set()
            return original_wait(*args)

        def close():
            generator.close()
            finished.set()

        with patch("shape._generator.lib") as native, patch.object(generator._cond, "wait", wait):
            closer = threading.Thread(target=close)
            closer.start()
            try:
                self.assertTrue(waiting.wait(5), "close did not wait for active call")
                self.assertTrue(generator.closed)
                self.assertFalse(finished.is_set())
                with self.assertRaises(shape.ShapeError):
                    generator.schema()
                native.return_value.shape_generator_free.assert_not_called()
            finally:
                with generator._cond:
                    generator._active = 0
                    generator._cond.notify_all()
                closer.join(5)
            self.assertFalse(closer.is_alive())
            generator.close()
            native.return_value.shape_generator_free.assert_called_once_with(123)
        with self.assertRaises(shape.ShapeError):
            generator.__enter__()

    def test_method_cannot_be_truncated_at_nul(self):
        generator = shape.Generator(123)
        try:
            with patch("shape._generator.lib") as native:
                with self.assertRaises(shape.ShapeError) as caught:
                    generator.call("schema\0ignored")
                self.assertEqual(caught.exception.code, "invalid_request")
                native.assert_not_called()
        finally:
            with patch("shape._generator.lib"):
                generator.close()


class StatelessTests(unittest.TestCase):
    def test_version(self):
        version = shape.version()
        self.assertRegex(version, r"^\d+\.\d+\.\d+")
        value = shape.call("version")
        self.assertEqual(value["version"], version)
        self.assertIn("cleanup.thread", value["commands"])
        self.assertEqual(shape.commands(), value["commands"])

    def test_unknown_command(self):
        with self.assertRaises(shape.ShapeError) as caught:
            shape.call("no.such.command")
        self.assertEqual(caught.exception.code, "unknown_command")
        self.assertIn("no.such.command", caught.exception.message)
        self.assertIsInstance(caught.exception, Exception)

    def test_missing_field(self):
        with self.assertRaises(shape.ShapeError) as caught:
            shape.call("deob")
        self.assertEqual(caught.exception.code, "invalid_request")
        self.assertIn("source", caught.exception.message)

    def test_wrong_field_type(self):
        with self.assertRaises(shape.ShapeError) as caught:
            shape.call("cleanup.thread", source=12)
        self.assertEqual(caught.exception.code, "invalid_request")

    def test_error_str(self):
        error = shape.ShapeError("failed", "boom")
        self.assertEqual(str(error), "failed: boom")
        self.assertEqual((error.code, error.message), ("failed", "boom"))

    def test_cleanup_thread(self):
        out = shape.cleanup.thread(THREAD)
        self.assertIn("return a", out["code"])
        self.assertNotIn("$SSA_1", out["code"])
        self.assertEqual(out["cleanup"]["inlined_bindings"], 1)

    def test_cleanup_thread_from_threads(self):
        results = [None] * 4

        def work(i):
            results[i] = shape.cleanup.thread(THREAD)["code"]

        threads = [threading.Thread(target=work, args=(i,)) for i in range(len(results))]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(len(set(results)), 1)
        self.assertIn("return a", results[0])

    def test_unparsable_document_string(self):
        with self.assertRaises(shape.ShapeError) as caught:
            shape.compact_schema("{not json")
        self.assertEqual(caught.exception.code, "invalid_request")

    def test_generator_load_error(self):
        with self.assertRaises(shape.ShapeError) as caught:
            shape.Generator.load(b"{}")
        self.assertIn(caught.exception.code, ("failed", "invalid_request"))
        with self.assertRaises(shape.ShapeError):
            shape.Generator.load(42)


ARTIFACT = os.environ.get("SHAPE_TEST_ARTIFACT")
HEADERS = os.environ.get("SHAPE_TEST_HEADERS")


@unittest.skipUnless(
    ARTIFACT and HEADERS, "set SHAPE_TEST_ARTIFACT and SHAPE_TEST_HEADERS to run"
)
class GeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.artifact = Path(ARTIFACT).read_bytes()
        cls.headers = json.loads(Path(HEADERS).read_text())
        cls.generator = shape.Generator.load(cls.artifact)

    @classmethod
    def tearDownClass(cls):
        cls.generator.close()

    def test_decode(self):
        decoded = self.generator.decode(self.headers)
        signals = decoded["positionalSignals"]
        self.assertEqual(signals["DeviceMemory"]["deviceMemory"], 32)
        self.assertGreater(len(signals), 0)
        self.assertEqual(decoded["metadata"]["decoded"], len(signals))

    def test_decode_accepts_json_text(self):
        decoded = self.generator.decode(json.dumps(self.headers))
        self.assertEqual(decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"], 32)

    def test_artifact_object_preserves_large_integers(self):
        # Artifacts carry 64-bit literals (f64 bit patterns). Loading the
        # artifact as a parsed dict makes the binding dump it again, so the
        # request path must keep those ints exact; the decoded payloads are
        # the same only if it did.
        artifact = json.loads(self.artifact)
        artifact_big = list(unsafe_integers(artifact))
        if not artifact_big:
            self.skipTest("this artifact has no integer above 2^53")
        with shape.Generator.load(artifact) as generator:
            expected = self.generator.decode(self.headers, metadata=True)
            actual = generator.decode(self.headers, metadata=True)
        self.assertEqual(actual, expected)
        for path, value in unsafe_integers(actual):
            self.assertIs(type(value), int, path)
        if list(unsafe_integers(expected)):
            self.assertTrue(
                list(unsafe_integers(actual)),
                "decoded payload lost its 64-bit integers",
            )

    def test_explain(self):
        decoded = self.generator.decode(self.headers)
        explained = self.generator.explain(decoded, "DeviceMemory")
        self.assertEqual(explained["status"], "decoded")
        explained = self.generator.explain(json.dumps(decoded), "DeviceMemory")
        self.assertEqual(explained["status"], "decoded")

    def test_schema(self):
        schema = self.generator.schema()
        self.assertIsInstance(schema, dict)
        self.assertTrue(schema)
        self.assertEqual(schema, shape.compact_schema(self.artifact))

    def test_encode_round_trips_and_accepts_edits(self):
        decoded = self.generator.decode(self.headers, metadata=True)
        self.assertEqual(self.generator.encode(decoded=decoded), self.headers)
        edited = json.loads(json.dumps(decoded))
        edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
        headers = self.generator.encode(decoded=edited)
        again = self.generator.decode(headers)
        self.assertEqual(again["positionalSignals"]["DeviceMemory"]["deviceMemory"], 8)

    def test_encode_document_forms_and_exclusivity(self):
        decoded = self.generator.decode(self.headers, metadata=True)
        for value in (json.dumps(decoded), json.dumps(decoded).encode("utf-8")):
            self.assertEqual(self.generator.encode(decoded=value), self.headers)
        for request in ({}, {"decoded": decoded, "signals": decoded["positionalSignals"]}):
            with self.assertRaises(shape.ShapeError) as caught:
                self.generator.encode(**request)
            self.assertEqual(caught.exception.code, "invalid_request")

    def test_load_forms(self):
        for artifact in (self.artifact.decode("utf-8"), json.loads(self.artifact), Path(ARTIFACT)):
            with shape.Generator.load(artifact) as generator:
                self.assertEqual(
                    generator.decode(self.headers)["positionalSignals"]["DeviceMemory"]["deviceMemory"],
                    32,
                )
            self.assertTrue(generator.closed)
            with self.assertRaises(shape.ShapeError):
                generator.schema()

    def test_concurrent_decode(self):
        results, errors = [], []

        def work():
            try:
                for _ in range(3):
                    results.append(
                        self.generator.decode(self.headers)["positionalSignals"]["DeviceMemory"]["deviceMemory"]
                    )
            except Exception as error:  # surfaced below
                errors.append(error)

        threads = [threading.Thread(target=work) for _ in range(2)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(errors, [])
        self.assertEqual(results, [32] * 6)


if __name__ == "__main__":
    unittest.main()
