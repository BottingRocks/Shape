# CorsCookieValue

[Signals index](README.md) · [data-shape example](cors_cookie_value.js) · [schema format](schema.md)

## What it collects

Reads document.cookie once, looks for the cookie named NyJB5Jgn and sends its value as an object { value }. If the cookie is not there it sends an empty object. The value is not trimmed or URL-decoded and is cut at its first '='. The collector is registered as custom_corsCookie. It never writes a cookie.

## Why

The NyJB5Jgn cookie is likely set by another vendor script or server response; reading it back likely links this session to an earlier visit or confirms that a cross-origin/server step really ran (the purpose is not visible in the source).

## Fields

Sent by name through the generic encoder: the value is handed to the shared encoder as one named object (or bare value), not written field by field with a positional index.

This signal sends one of 2 message shapes.

### Message 1: a NyJB5Jgn cookie exists

| Field | Type | Meaning |
| --- | --- | --- |
| `value` | string | The cookie's value as raw text (up to the first '='); undefined if the cookie has no '='. |

### Message 2: no NyJB5Jgn cookie

No fields: the message is empty.

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CorsCookieValue` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Sent **by name** through the generic encoder: `type: "name"` with the build's routing `name` (no `positional`), signal `mask: null`; each value is typed `"value"` and keeps its wire `key`.
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
