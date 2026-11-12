"""Cross-language contract verification; standard library orchestration only."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LANGUAGES = ("rust", "c", "node", "python", "go")


def run(argv, *, env=None, cwd=None):
    result = subprocess.run(list(map(str, argv)), env=env, cwd=cwd, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(map(str, argv))}\n"
                           f"{result.stdout}\n{result.stderr}")
    return result.stdout


def dump(path, value):
    path.write_text(json.dumps(value, allow_nan=False))
    return path


def check_types(temp, node_root, operations):
    """Compile actual exported named paths in a strict standalone ESM consumer."""
    types = temp / "types"
    modules = types / "node_modules"
    modules.mkdir(parents=True)
    (modules / "shape").symlink_to(temp / "node", target_is_directory=True)
    dependencies = node_root / "sdk/node/node_modules"
    if (dependencies / "@types").is_dir():
        (modules / "@types").symlink_to(dependencies / "@types", target_is_directory=True)
    compiler = dependencies / "typescript/bin/tsc"
    command = ["node", compiler] if compiler.is_file() else ["tsc"]
    if not compiler.is_file() and not shutil.which("tsc"):
        raise RuntimeError("missing TypeScript compiler; install sdk/node devDependencies")
    shutil.copy2(HERE / "types.mts", types / "types.mts")
    # Import the same public-path mapping used by runtime coverage; required IDs
    # come solely from the provider registry, never a second inventory.
    script = ("import {publicPath} from " + json.dumps((HERE / "named.mjs").as_uri()) + ";"
              "console.log(JSON.stringify(JSON.parse(process.argv[1]).map(publicPath)))")
    paths = json.loads(run(["node", "--input-type=module", "-e", script, json.dumps(operations)]))
    (types / "surface.mts").write_text("import * as shape from 'shape';\nconst methods = [\n" +
        ",\n".join(f"  shape.{path}" for path in paths) +
        "\n] satisfies readonly ((...args: never[]) => unknown)[];\nvoid methods;\n")
    run([*command, "--noEmit", "--strict", "--noUncheckedIndexedAccess", "--verbatimModuleSyntax",
         "--typeRoots", dependencies / "@types",
         "--module", "NodeNext", "--moduleResolution", "NodeNext", "--target", "ES2022",
         types / "types.mts", types / "surface.mts"])
    print(f"TYPES strict ESM value/type imports and {len(operations)} public operation paths", flush=True)


def prepare(temp, release, *, node_root=None, python_root=None, go_root=None, provider_root=None):
    node_root = node_root or ROOT
    python_root = python_root or ROOT
    go_root = go_root or ROOT
    provider_root = provider_root or ROOT
    for command in ("uv", "node", "go", "rustc", "cc"):
        if not shutil.which(command):
            raise RuntimeError(f"missing prerequisite: {command}; see sdk/tests/README.md")
    library = release / ("libshape.dylib" if sys.platform == "darwin" else "libshape.so")
    if not library.is_file():
        raise RuntimeError(f"missing {library}; run scripts/build-sdk.sh first")
    # Pin one snapshot: never write to the supplied release directory.
    lib = temp / "lib"
    lib.mkdir()
    shutil.copy2(library, lib / library.name)
    env = dict(os.environ, SHAPE_LIBRARY=str(lib / library.name))
    # Public matching snapshots already cover sample32 and exact 64-bit metadata.
    # Ordinary verification needs neither seed recovery nor fixture generation.
    fixture = ROOT / "sdk/c/tests/fixtures"
    artifact = temp / "artifact.json"
    shutil.copy2(fixture / "artifact.json", artifact)
    capture = json.loads((fixture / "capture.json").read_text())
    env["CONTRACT_PYTHON_ROOT"] = str(python_root / "sdk/python")

    node_dir = temp / "node"
    shutil.copytree(node_root / "sdk/node", node_dir,
                    ignore=shutil.ignore_patterns("build", "lib", "node_modules"))
    run(["sh", node_dir / "build.sh"], env=dict(env, SHAPE_LIBRARY_DIR=str(lib),
        SHAPE_INCLUDE_DIR=str(provider_root / "sdk/c/include")))
    env["CONTRACT_NODE_MODULE"] = str(node_dir / "index.mjs")

    c = temp / "c-runner"
    run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror", HERE / "runner.c",
         "-I", provider_root / "sdk/c/include", "-L", lib, "-lshape",
         f"-Wl,-rpath,{lib}", "-o", c])

    # Root shape is a native rlib built as a dependency of shape-c. Reuse it
    # read-only, including arbitrary_precision, instead of rebuilding Oxc.
    dependencies = release / "deps"
    rlibs = list(dependencies.glob("libshape-*.rlib"))
    if len(rlibs) != 1:
        raise RuntimeError(f"expected exactly one native shape rlib in {dependencies}; "
                           "use a clean release build (scripts/build-sdk.sh)")
    rust = temp / "rust-runner"
    json_rlibs = list(dependencies.glob("libserde_json-*.rlib"))
    if len(json_rlibs) != 1:
        raise RuntimeError(f"expected exactly one matching serde_json rlib in {dependencies}")
    run(["rustc", "--edition=2021", HERE / "runner.rs", "--extern", f"shape={rlibs[0]}",
         "--extern", f"serde_json={json_rlibs[0]}",
         "-L", f"dependency={dependencies}", "-L", f"native={dependencies}", "-o", rust])

    go_dir = temp / "go"
    go_dir.mkdir()
    shutil.copy2(HERE / "runner.go", go_dir / "main.go")
    shutil.copy2(HERE / "named.go", go_dir / "named.go")
    (go_dir / "go.mod").write_text(
        f"module contract\n\ngo 1.22\n\nrequire shape v0.0.0\n"
        f"replace shape => {json.dumps(str(go_root / 'sdk/go'))}\n")
    go = temp / "go-runner"
    go_env = dict(env, CGO_ENABLED="1", CGO_LDFLAGS=f"-L{lib} -lshape -Wl,-rpath,{lib}",
                  CGO_CFLAGS=f"-I{provider_root / 'sdk/c/include'}")
    run(["go", "build", "-tags", "shape_external", "-o", go, "."], env=go_env, cwd=go_dir)
    commands = {"rust": [rust], "c": [c], "node": ["node", HERE / "runner.mjs"],
                "python": ["uv", "run", "--no-project", "python", HERE / "runner.py"], "go": [go]}
    return artifact, capture, env, commands


def same_json(left, right):
    """Object order is immaterial; booleans must never equal JSON numbers."""
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is type(right) and left == right
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(same_json(left[k], right[k]) for k in left)
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(same_json(a, b) for a, b in zip(left, right))
    return left == right


def large_integers(value):
    if type(value) is int and abs(value) > 2**53:
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from large_integers(child)
    elif isinstance(value, list):
        for child in value:
            yield from large_integers(child)


class Contract:
    def __init__(self, temp, artifact, env, commands):
        self.temp, self.artifact, self.env, self.commands = temp, artifact, env, commands
        self.cases = self.checks = self.skips = 0
        self.failures = []

    def case(self, name, method, body, *, scope="generator", error=None, raw=False,
             languages=LANGUAGES):
        self.cases += 1
        request = self.temp / "request.json"
        if raw:
            request.write_text(body)
        else:
            dump(request, body)
        results = {}
        for language in languages:
            if raw and language == "python":
                # Python's generic API is **kwargs, not a raw request-text API.
                print(f"SKIP {name}/python: public generic API accepts keyword fields only")
                self.skips += 1
                continue
            argv = [*self.commands[language], self.artifact, scope, method, request]
            if raw and language == "node":
                argv.append("raw")
            envelope = json.loads(run(argv, env=self.env))
            dump(self.temp / f"{name}.{language}.json", envelope)
            self.checks += 1
            if error:
                if envelope.get("ok") is not False or envelope.get("error", {}).get("code") != error:
                    self.fail(f"{name}/{language}: expected error {error}, got {envelope}")
                # Human-oriented error wording is not a stable contract; compare
                # machine-readable error code explicitly, never suppress values.
                results[language] = {"ok": envelope.get("ok"), "code": envelope.get("error", {}).get("code")}
            else:
                if envelope.get("ok") is not True:
                    self.fail(f"{name}/{language}: expected success, got {envelope}")
                results[language] = envelope
        reference = "rust" if "rust" in results else languages[0]
        baseline = results[reference]
        for language, actual in results.items():
            if not same_json(actual, baseline):
                self.fail(f"{name}/{language}: differs from native Rust; "
                                     f"inspect {name}.{{{reference},{language}}}.json")
        print(f"CASE {name}: {len(results)} runtimes", flush=True)
        if not error:
            if baseline.get("ok") is not True:
                raise RuntimeError(f"cannot continue dependent cases after {name}: {baseline}")
            return baseline["value"]

    def expect(self, condition, message):
        if not condition:
            self.fail(message)

    def fail(self, message):
        self.failures.append(message)
        # Preserve mismatches even if a later compiler/driver failure aborts.
        print(f"FAIL {message}", file=sys.stderr, flush=True)


def exercise(contract, capture):
    c = contract
    session = json.loads((ROOT / "tests/fixtures/sdk/decode/session.json").read_text())
    version = c.case("version", "version", {}, scope="stateless")
    c.expect("cleanup.thread" in version["commands"], "version catalogue missing cleanup.thread")
    c.case("cleanup", "cleanup.thread", {"source": "function f(a) { let $SSA_1 = a; return $SSA_1; }"}, scope="stateless")
    c.case("unknown-command", "contract.unknown", {}, scope="stateless", error="unknown_command")
    c.case("bad-source", "cleanup.thread", {"source": 12}, scope="stateless", error="invalid_request")
    schema = c.case("schema", "schema", {})
    c.expect(bool(schema), "schema is empty")
    decoded = c.case("decode-default", "decode", {"capture": capture})
    signals = decoded["positionalSignals"]
    c.expect(signals["DeviceMemory"]["deviceMemory"] == 32, "sample must decode DeviceMemory=32")
    c.expect(bool(decoded["metadata"]["originalReport"]), "default decode lost retained originalReport")
    detailed = c.case("decode-metadata", "decode", {"capture": capture, "metadata": True})
    integers = list(large_integers(detailed))
    c.expect(bool(integers), "detailed metadata must exercise integer literals above 2^53")
    print(f"PRECISION {len(integers)} integers above 2^53 in detailed decode", flush=True)
    c.case("decode-verify", "decode", {"capture": capture, "verify": True})
    text = c.case("decode-text", "decode", {"capture": json.dumps(capture)})
    c.expect(text == decoded, "capture JSON text changed default decode")
    for name, supplied in (("object", session), ("text", json.dumps(session))):
        result = c.case(f"decode-session-{name}", "decode", {"capture": capture, "session": supplied})
        c.expect(result == decoded, f"session {name} changed decode")
    c.case("explain", "explain", {"decoded": decoded, "signal": "DeviceMemory", "verify": True})
    c.case("explain-text", "explain", {"decoded": json.dumps(decoded), "signal": "DeviceMemory", "session": json.dumps(session)})
    for name, payload in (("default", decoded), ("detailed", detailed), ("text", json.dumps(decoded))):
        encoded = c.case(f"encode-full-{name}", "encode", {"decoded": payload})
        c.expect(encoded == capture, f"unchanged full {name} encode did not preserve exact header map")
    edited = copy.deepcopy(decoded)
    edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
    encoded = c.case("encode-edited", "encode", {"decoded": edited, "session": session})
    again = c.case("decode-edited", "decode", {"capture": encoded})
    expected = copy.deepcopy(signals)
    expected["DeviceMemory"]["deviceMemory"] = 8
    c.expect(again["positionalSignals"] == expected, "edited full encode changed signals beyond DeviceMemory=8")
    # Signals-only encryption may choose fresh prefixes. Compare the entire
    # decoded positionalSignals, never ciphertext or retained originalReport.
    # Each runtime's own encoded capture must be decoded by every runtime.
    for language in LANGUAGES:
        request = dump(c.temp / "request.json", {"signals": expected, "session": json.dumps(session)})
        envelope = json.loads(run([*c.commands[language], c.artifact, "generator", "encode", request], env=c.env))
        c.checks += 1
        c.expect(envelope.get("ok") is True, f"signals-only encode/{language}: {envelope}")
        if envelope.get("ok"):
            result = c.case(f"signals-from-{language}", "decode", {"capture": envelope["value"]})
            c.expect(result["positionalSignals"] == expected, f"signals-only/{language} changed decoded signals")
    wrong = copy.deepcopy(capture)
    wrong["siteId"] = "contract-wrong-site"
    c.case("wrong-site", "decode", {"capture": wrong}, error="invalid_request")
    wrong_payload = copy.deepcopy(decoded)
    wrong_payload["siteId"] = "contract-wrong-site"
    for name, payload in (("object", wrong_payload), ("text", json.dumps(wrong_payload))):
        c.case(f"explain-wrong-site-{name}", "explain",
               {"decoded": payload, "signal": "DeviceMemory"}, error="invalid_request")
        c.case(f"encode-wrong-site-{name}", "encode", {"decoded": payload}, error="invalid_request")
    for name, payload in (
        ("nonstring-site", dict(decoded, siteId=12)),
        ("nonobject-signals", dict(decoded, positionalSignals=[])),
        ("unknown-signal", dict(decoded, positionalSignals={"NoSuchSignal": {}})),
        ("nonobject-fields", dict(decoded, positionalSignals={"DeviceMemory": 32})),
        ("invalid-report", dict(decoded, metadata={"originalReport": []})),
    ):
        c.case(f"encode-full-{name}", "encode", {"decoded": payload}, error="invalid_request")
    for field in ("decoded", "signals"):
        for name, payload in (("malformed", "{"), ("nonobject", [])):
            c.case(f"encode-{field}-{name}", "encode", {field: payload}, error="invalid_request")
    c.case("unknown-method", "contract.unknown", {}, error="unknown_command")
    c.case("malformed-document", "decode", {"capture": "{"}, error="invalid_request")
    c.case("nonobject-document", "decode", {"capture": []}, error="invalid_request")
    c.case("exclusive-input", "encode", {"signals": signals, "decoded": decoded}, error="invalid_request")
    c.case("missing-input", "encode", {}, error="invalid_request")
    c.case("bad-verify", "decode", {"capture": capture, "verify": "yes"}, error="invalid_request")
    for scope, method in (("stateless", "version"), ("generator", "schema")):
        c.case(f"malformed-request-{scope}", method, "{", scope=scope, error="invalid_request", raw=True)
        c.case(f"nonobject-request-{scope}", method, "[]", scope=scope, error="invalid_request", raw=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-dir", type=Path, default=ROOT / "target/release",
                        help="read-only release directory from scripts/build-sdk.sh")
    parser.add_argument("--output", type=Path, help="retain outputs in this new directory (default: temporary)")
    for binding in ("node", "python", "go", "provider"):
        parser.add_argument(f"--{binding}-root", type=Path, default=ROOT,
                            help=f"checkout containing the {binding} SDK (default: this checkout)")
    parser.add_argument("--registry", type=Path, help="provider operation registry (default: provider-root/sdk/operations.json)")
    parser.add_argument("--legacy-only", action="store_true", help="run the existing compatibility cases only")
    args = parser.parse_args()
    context = tempfile.TemporaryDirectory(prefix="shape-contract-") if args.output is None else None
    temp = Path(context.name) if context else args.output.resolve()
    if not context:
        temp.mkdir(parents=True, exist_ok=False)
    try:
        artifact, capture, env, commands = prepare(
            temp, args.release_dir.resolve(), **{
                f"{binding}_root": getattr(args, f"{binding}_root").resolve()
                for binding in ("node", "python", "go", "provider")
            })
        contract = Contract(temp, artifact, env, commands)
        exercise(contract, capture)
        if not args.legacy_only:
            import portable
            inventory = args.registry or args.provider_root / "sdk/operations.json"
            _, operations = portable.registry(inventory)
            check_types(temp, args.node_root.resolve(), operations)
            portable.exercise(contract, capture, ROOT, inventory)
        print(f"RESULT {contract.cases} cases, {contract.checks} runtime checks, "
              f"{contract.skips} explicit skips, {len(contract.failures)} failures")
        for failure in contract.failures:
            print(f"FAIL {failure}", file=sys.stderr)
        return bool(contract.failures)
    finally:
        if context:
            context.cleanup()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, OSError, ValueError) as error:
        sys.exit(f"contract harness: {error}")
