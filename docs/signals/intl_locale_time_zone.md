# IntlLocaleTimeZone

[Signals index](README.md) · [data-shape example](intl_locale_time_zone.js) · [schema format](schema.md)

## What it collects

Reads the browser's internationalization (Intl) setup. It asks four Intl services (DateTimeFormat, Collator, DisplayNames, Segmenter) which default locale they use, hashes the time-zone name, reads the UTC offset, and counts how many calendars, collations, currencies, numbering systems, time zones and units the engine supports.

## Why

Locale and time zone can be checked against the claimed IP location and the Accept-Language header. The supported-value counts identify the browser's ICU/CLDR data version, which exposes spoofed or mismatched browser builds (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `calendarCount` | number | Number of calendars from Intl.supportedValuesOf("calendar"), or -1 if unsupported. |
| `segmenterLocale` | string or null (tag byte) | Default locale of Intl.Segmenter, e.g. "en-US", or a null marker if Intl.Segmenter is missing or throws. |
| `displayNamesLocale` | string or null (tag byte) | Default locale of Intl.DisplayNames, or a null marker if it is missing or throws. |
| `unitCount` | number | Number of units from Intl.supportedValuesOf("unit"), or -1 if unsupported. |
| `dateTimeFormatLocale` | string or null (tag byte) | Default locale of Intl.DateTimeFormat, or a null marker if it throws. |
| `timeZoneHash` | number | 32-bit hash of the time-zone name (e.g. "Europe/Berlin"), or -1 if it cannot be read. |
| `timeZoneCount` | number | Number of time zones from Intl.supportedValuesOf("timeZone"), or -1 if unsupported. |
| `collatorLocale` | string or null (tag byte) | Default locale of Intl.Collator, or a null marker if it throws. |
| `timezoneOffsetMin` | number | new Date().getTimezoneOffset(): minutes between local time and UTC (positive west of UTC, e.g. 300 for US Eastern winter time, -60 for Central Europe). |
| `currencyCount` | number | Number of currencies from Intl.supportedValuesOf("currency"), or -1 if unsupported. |
| `numberingSystemCount` | number | Number of numbering systems from Intl.supportedValuesOf("numberingSystem"), or -1 if unsupported. |
| `collationCount` | number | Number of collations from Intl.supportedValuesOf("collation"), or -1 if unsupported. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "calendarCount": 18,
  "collationCount": 10,
  "collatorLocale": "en-US",
  "currencyCount": 159,
  "dateTimeFormatLocale": "en-US",
  "displayNamesLocale": "en-US",
  "numberingSystemCount": 78,
  "segmenterLocale": "en-US",
  "timeZoneCount": 418,
  "timeZoneHash": 146801107,
  "timezoneOffsetMin": 480,
  "unitCount": 45
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.IntlLocaleTimeZone` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
