"""Public named API adapter; never falls back to generic dispatch."""

ALIASES = {"seeds.import": "seeds.import_seed"}


def public_path(operation):
    return ALIASES.get(operation, operation)


def resolve(shape, operation):
    value = shape
    for part in public_path(operation).split("."):
        value = getattr(value, part)
    if not callable(value):
        raise TypeError(f"named API is not callable: {operation}")
    return value


def invoke(shape, operation, body):
    fn = resolve(shape, operation)
    body = dict(body)
    if "cache" in body and body["cache"] is not None:
        policy = body["cache"]
        mode = policy["mode"]
        body["cache"] = (shape.CachePolicy.directory(policy["directory"]) if mode == "directory"
                         else shape.CachePolicy(mode))
    if operation == "artifact.compact_schema":
        return fn(body["artifact"])
    if operation == "artifact.compile":
        with fn(**body) as loaded:
            return loaded.artifact()
    if operation == "signals.describe":
        return fn(body["name"])
    if operation == "headers.parse":
        return fn(body["text"])
    if operation in ("seeds.parse", "seeds.parse_launcher", "seeds.parse_kernel"):
        return fn(bytes(body["bytes"]))
    if operation == "seeds.parse_url":
        return fn(body["input"])
    if operation in ("unpack", "unpack_canonical"):
        return fn(bytes(body["bytes"]), body.get("options"))
    if operation == "pack":
        return list(fn(body["value"], body.get("options")))
    if operation == "payload.encode_bytes":
        return fn(body["profile"], bytes(body["bytes"]), body["seeds"])
    # Python namespace keyword parameters follow the canonical wire fields.
    return fn(**body)


def loaded_call(shape, loaded, operation, body):
    if operation == "artifact":
        return loaded.artifact()
    if operation == "encode" and "signals" in body:
        return shape.encode_signals(loaded, **body)
    return getattr(shape, operation)(loaded, **body)
