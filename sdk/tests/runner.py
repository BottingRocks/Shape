"""Exercise public Python binding only; host argument types are out of scope."""
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, os.environ.get("CONTRACT_PYTHON_ROOT", str(Path(__file__).resolve().parents[1] / "python")))
import shape
from named import invoke, loaded_call, resolve

artifact, scope, method, request = sys.argv[1:]
try:
    environment = dict(os.environ)
    body = json.loads(Path(request).read_text())
    if scope == "surface":
        for operation in body:
            resolve(shape, operation)
        for operation in ("decode", "encode", "encode_signals", "explain", "schema"):
            assert callable(getattr(shape, operation))
        value = body
    elif scope == "named":
        value = invoke(shape, method, body)
    elif scope == "kernel":
        with shape.kernel.load(json.loads(Path(artifact).read_text())) as loaded:
            value = shape.trace.program_with_artifact(body["source"], loaded)
    elif scope == "loaded":
        with shape.artifact.load(json.loads(Path(artifact).read_text())) as loaded:
            detached = loaded.artifact()
            detached["siteId"] = "mutated-export-must-not-change-loaded-state"
            value = loaded_call(shape, loaded, method, body)
            for _ in range(0 if method == "encode" else 2):
                assert value == loaded_call(shape, loaded, method, body), "loaded operation changed on reuse"
    elif scope == "generator":
        with shape.Generator.load(json.loads(Path(artifact).read_text())) as generator:
            value = generator.call(method, **body)
    else:
        value = shape.call(method, **body)
    assert environment == dict(os.environ), "SDK mutated process environment"
    result = {"ok": True, "value": value}
except shape.ShapeError as error:
    result = {"ok": False, "error": {"code": error.code, "message": error.message}}
print(json.dumps(result, allow_nan=False))
