# ProductPageSnapshot

[Signals index](README.md) · [data-shape example](product_page_snapshot.js) · [schema format](schema.md)

## What it collects

In the collection step, reads six pieces of the current product page with fixed CSS selectors and sends them as one object {a, b, c, d, e, f}: the page title, the add-to-cart button's label, the product title, the selected store name, the item id from the data-tcin attribute, and a 32-bit hash of the product's specification labels. Three texts are cut to their first 20 characters. A missing element gives the empty string. It reads the page only; nothing is changed.

## Why

Likely records which product and store the session is looking at when it sends data, so that traffic hitting one item (for example bots watching stock or buying up a limited product) can be grouped and told apart from normal shopping. The specifications hash lets the server check that the page content matches the item id without sending the full text (the use is inferred).

## Fields

Sent by name through the generic encoder: the value is handed to the shared encoder as one named object (or bare value), not written field by field with a positional index.

| Field | Type | Meaning |
| --- | --- | --- |
| `pageTitle` | string | Wire key a: the first 20 characters of document.querySelector('title').outerText; "" if there is no &lt;title&gt;. |
| `addToCartLabel` | string | Wire key b: the first 20 characters of the aria-label (ariaLabel) of the first button whose id starts with "addToCart"; "" if there is no such button. |
| `productTitle` | string | Wire key c: the first 20 characters of the outerText of h1[data-test="product-title"] &gt; span; "" if absent. |
| `storeName` | string | Wire key d: the full outerText of the first button whose id starts with "store-name" (the chosen store); "" if absent. |
| `itemId` | string | Wire key e: the data-tcin attribute (the retailer's item number) of the first div that has one; "" if absent (the -1 default is always overwritten). |
| `specificationsHash` | number | Wire key f: signed 32-bit hash (h = h*31 + pair of UTF-16 units) of the joined first-child text of every div[data-test="item-details-specifications"] &gt; div &gt; div &gt; b label; 0 when there are none (hash of the empty string). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ProductPageSnapshot` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Sent **by name** through the generic encoder: `type: "name"` with the build's routing `name` (no `positional`), signal `mask: null`; each value is typed `"value"` and keeps its wire `key`.
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
