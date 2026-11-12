# NavigatorPlugins

[Signals index](README.md) · [data-shape example](navigator_plugins.js) · [schema format](schema.md)

## What it collects

Summarises navigator.plugins. For each of the first five plugins it sends the first 12 characters of the plugin's name and a hash of all its details (name, description, filename, version and the type, suffixes and description of each of its MIME types). It also sends how many plugins it summarised and the full navigator.plugins.length. A plugin whose details cannot be read is skipped.

## Why

The plugin list is a classic browser fingerprint: real Chrome, Edge, Firefox and Safari each expose a known fixed list (e.g. the five PDF viewer entries), while headless browsers and some bots likely expose none or a fake list.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `entryCount` | number | Number of plugin entries that follow, normally min(plugins.length, 5); can be lower if a plugin could not be read. |
| `plugin` | group, repeated (once per summarised plugin, entryCount times (last plugin first)) | One entry per repetition with the fields below. |
| `plugin[].namePrefix` | string | First 12 characters of the plugin name, e.g. "PDF Viewer" or "Chrome PDF V". |
| `plugin[].detailsHash` | number | Signed 32-bit hash of the plugin's name, description, filename, version and its MIME types' type/suffixes/description, serialised as JSON. |
| `pluginsLength` | number | navigator.plugins.length, not capped (0 in many headless browsers, 5 in current desktop Chrome). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "plugin": [
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    },
    {
      "detailsHash": 1387368353,
      "namePrefix": "Microsoft Ed"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    }
  ],
  "pluginsLength": 5
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NavigatorPlugins` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
