# RememberMeCheckboxState

[Signals index](README.md) · [data-shape example](remember_me_checkbox_state.js) · [schema format](schema.md)

## What it collects

At the last moment before the data is sent, reads the state of the login form's "remember me" checkbox and sends it as one bare value (not an object). It looks for an element with id rememberMe, then id TravelerLoginForm_rememberMe, and sends its checked property. If neither exists it falls back to the first element named shouldRememberUser and sends whether its class name contains "checked" (a styled, non-native checkbox). If none of these exist it sends the empty string. It reads the page only; nothing is changed.

## Why

Likely ties the request to a login attempt and records whether the user asked to stay signed in. Credential-stuffing tools usually submit the login form without touching that box, so its state helps separate scripted logins from real ones (the use is inferred; the source only collects the value).

## Fields

Sent by name through the generic encoder: the value is handed to the shared encoder as one named object (or bare value), not written field by field with a positional index.

| Field | Type | Meaning |
| --- | --- | --- |
| `rememberMe` | boolean (tag byte) | Boolean: the checked state of #rememberMe (or #TravelerLoginForm_rememberMe); otherwise whether the first [name=shouldRememberUser] element's className contains "checked". The empty string "" (a STRING) when none of these elements is on the page. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.RememberMeCheckboxState` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Sent **by name** through the generic encoder: `type: "name"` with the build's routing `name` (no `positional`), signal `mask: null`; each value is typed `"value"` and keeps its wire `key`.
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
