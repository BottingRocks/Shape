"""Generate local Python/Go SDK fixtures using a release libshape.

From the repo root, set SHAPE_LIBRARY to a release library, then run:
    uv run --no-project python sdk/python/tests/generate_fixtures.py
Set SHAPE_TEST_ARTIFACT and SHAPE_TEST_HEADERS to the printed paths.
Outputs stay in the checkout's ignored sdk/python/artifacts directory.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import shape  # noqa: E402


def main():
    root = Path(__file__).resolve().parents[3]
    reference = root / "src/signals/wire/reference"
    # Match the certified integer-writer setup in tests/decode_workflow.rs.
    source = (root / "tests/fixtures/signal-wire-roundtrip/program.js").read_text()
    source = source.replace(
        "__HASH__", (reference / "hash.js").read_text().replace("local0(", "tokenHash(")
    ).replace(
        "__STREAM__",
        (reference / "stream.js").read_text()
        .replace("local0(", "streamFactory(")
        .replace("local9", "tokenState"),
    )
    writer = (reference / "number.js").read_text().replace("local0(", "integerWriter(")
    string_writer = (
        (reference / "string.js")
        .read_text()
        .replace("local0(", "stringFactory(")
        .replace("local10(", "integerWriter(")
        .replace("local15(", "codeUnit(")
    )
    helpers = (
        writer
        + "\n"
        + string_writer
        + "\nlet codeUnit=Array.call.bind(String.prototype.charCodeAt);"
    )
    source = source.replace("function main() {", "function main() {\n" + helpers, 1)
    source = source.replace(
        "function Session(token) {",
        "function Session(token) {\nlet sharedNumber; sharedNumber=integerWriter; "
        "const navigator={deviceMemory:32};",
    ).replace(
        "b[4]=9^mask();",
        "let callable; callable=sharedNumber; "
        "callable=callable(mask,navigator.deviceMemory,b,3); callable=0;",
    ).replace(
        "let registry=[{ctor:task,priority:false},{ctor:literal,priority:false}];",
        "let text=wrap(function(o) {\n"
        "    let seed=Math.random()*4294967296|0;\n"
        "    let mask=streamFactory(seed^17);\n"
        "    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];\n"
        "    let write=stringFactory(mask);\n"
        "    write(\"\",b,3);\n"
        "    o.next([[],b]);\n"
        "  });\n"
        "  let registry=[{ctor:task,priority:false},{ctor:literal,priority:false},"
        "{ctor:text,priority:false}];",
    ).replace("let filter=[0,1];", "let filter=[0,1,2];")
    session = json.loads((root / "tests/fixtures/sdk/decode/session.json").read_text())
    artifact = shape.compile_artifact(source, session, "example")
    with shape.Generator.load(artifact) as generator:
        capture = generator.encode(signals={
            "DeviceMemory": {"deviceMemory": 32},
            "StorageAccessApiPresence": {"hasStorageAccess": True, "requestStorageAccess": False},
            "ConstantEmptyString": {"constant": ""},
        })
    output = root / "sdk/python/artifacts"
    output.mkdir(exist_ok=True)
    for name, value in (("artifact", artifact), ("headers", capture)):
        path = output / (name + ".json")
        path.write_text(json.dumps(value, allow_nan=False), encoding="utf-8")
        print(path)


if __name__ == "__main__":
    main()
