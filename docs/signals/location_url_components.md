# LocationUrlComponents

[Signals index](README.md) · [data-shape example](location_url_components.js) · [schema format](schema.md)

## What it collects

Reads the current page's address from window.location and sends its parts as text: the hostname, the full URL (cut to its first 100 characters), the protocol and the port. Nothing is hashed; the strings are sent as they are. It runs once when the collector starts.

## Why

Tells the vendor which page and site the collector is actually running on, so a token or payload replayed from another site, a local file or about:blank can be spotted. A hostname or port that is missing (not even an empty string) is likely a sign of an emulated or spoofed browser, since real browsers always return strings.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `hostname` | string or null (tag byte) | location.hostname, e.g. "www.example.com"; empty for file: or about:blank pages. Or a null marker if hostname is null/undefined (only in fake or emulated environments). |
| `href` | string | location.href, the full page URL, cut to its first 100 characters. Some builds leave this field out. |
| `protocol` | string | location.protocol, e.g. "https:", "http:" or "file:". |
| `port` | string or null (tag byte) | location.port as text, usually empty ("") for default ports, e.g. "8080" otherwise. Or a null marker if port is null/undefined (only in fake or emulated environments). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "hostname": "www.target.com",
  "href": "https://www.target.com/login?client_id=ecom-web-1.0.0&ui_namespace=ui-default&back_button_action=bro",
  "port": "",
  "protocol": "https:"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.LocationUrlComponents` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
