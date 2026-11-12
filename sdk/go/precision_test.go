package shape

// These tests pin down the JSON boundary without calling the native library:
// requests are marshalled exactly, response values are decoded with UseNumber
// (so `any` fields hold json.Number, never float64), and *Decoded keeps the
// exact document the library returned.

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

// A synthetic decoded payload: 2^53+1, the int64 minimum and the uint64
// maximum all appear where real artifacts/payloads carry 64-bit values.
const decodedDocument = `{
  "siteId": "example",
  "errors": [],
  "positionalSignals": {
    "Probe": {
      "numberBits": 9007199254740993,
      "signed": -9223372036854775808,
      "unsigned": 18446744073709551615
    }
  },
  "signals": [],
  "keyedSignals": [],
  "seeds": [4294967295, 1],
  "keysets": [],
  "metadata": {
    "originalReport": {},
    "signals": {},
    "decoded": 1,
    "missing": 0,
    "exactReencode": true,
    "floatByteOrder": "little"
  }
}`

func TestRequestJSONKeepsLargeIntegers(t *testing.T) {
	request := struct {
		Number   json.Number `json:"number"`
		Big      int64       `json:"big"`
		Unsigned uint64      `json:"unsigned"`
	}{
		Number:   json.Number("9007199254740993"),
		Big:      9223372036854775807,
		Unsigned: 18446744073709551615,
	}
	body, err := requestJSON(request)
	if err != nil {
		t.Fatal(err)
	}
	want := `{"number":9007199254740993,"big":9223372036854775807,"unsigned":18446744073709551615}`
	if body != want {
		t.Fatalf("requestJSON = %s, want %s", body, want)
	}

	// JSON text and raw messages pass through unchanged.
	for _, raw := range []any{
		json.RawMessage(`{"numberBits":9007199254740993}`),
		[]byte(`{"numberBits":9007199254740993}`),
		`{"numberBits":9007199254740993}`,
	} {
		body, err := requestJSON(raw)
		if err != nil {
			t.Fatalf("requestJSON(%T): %v", raw, err)
		}
		if body != `{"numberBits":9007199254740993}` {
			t.Fatalf("requestJSON(%T) = %s", raw, body)
		}
	}
}

func TestDecodeValueKeepsLargeIntegers(t *testing.T) {
	raw := json.RawMessage(`{"bits":9007199254740993,"signed":-9223372036854775808,` +
		`"unsigned":18446744073709551615,"nested":{"list":[9007199254740993]}}`)
	var out map[string]any
	if err := decodeValue("test", raw, &out); err != nil {
		t.Fatal(err)
	}
	for name, want := range map[string]string{
		"bits":     "9007199254740993",
		"signed":   "-9223372036854775808",
		"unsigned": "18446744073709551615",
	} {
		number, ok := out[name].(json.Number)
		if !ok {
			t.Fatalf("%s: %T, want json.Number (a float64 would have rounded it)", name, out[name])
		}
		if number.String() != want {
			t.Fatalf("%s = %s, want %s", name, number, want)
		}
	}
	nested, ok := out["nested"].(map[string]any)["list"].([]any)[0].(json.Number)
	if !ok || nested.String() != "9007199254740993" {
		t.Fatalf("nested value = %v, want json.Number 9007199254740993", out["nested"])
	}

	// Typed 64-bit fields decode without a float intermediate too.
	var typed struct {
		Big      int64  `json:"big"`
		Unsigned uint64 `json:"unsigned"`
	}
	if err := decodeValue("test", json.RawMessage(`{"big":9223372036854775807,"unsigned":18446744073709551615}`), &typed); err != nil {
		t.Fatal(err)
	}
	if typed.Big != 9223372036854775807 || typed.Unsigned != 18446744073709551615 {
		t.Fatalf("typed fields = %d, %d", typed.Big, typed.Unsigned)
	}
}

func TestUnwrapKeepsValueBytes(t *testing.T) {
	value, err := unwrap([]byte(`{"ok":true,"value":{"numberBits":9007199254740993,"unsigned":18446744073709551615}}`))
	if err != nil {
		t.Fatal(err)
	}
	var out map[string]any
	if err := decodeValue("value", value, &out); err != nil {
		t.Fatal(err)
	}
	if got := out["numberBits"].(json.Number).String(); got != "9007199254740993" {
		t.Fatalf("numberBits = %s", got)
	}
	if got := out["unsigned"].(json.Number).String(); got != "18446744073709551615" {
		t.Fatalf("unsigned = %s", got)
	}
}

func TestDecodedKeepsExactDocument(t *testing.T) {
	var decoded Decoded
	if err := json.Unmarshal([]byte(decodedDocument), &decoded); err != nil {
		t.Fatal(err)
	}
	if decoded.Seeds != [2]uint32{4294967295, 1} {
		t.Fatalf("seeds = %v", decoded.Seeds)
	}
	probe := decoded.PositionalSignals["Probe"]
	for name, want := range map[string]string{
		"numberBits": "9007199254740993",
		"signed":     "-9223372036854775808",
		"unsigned":   "18446744073709551615",
	} {
		number, ok := probe[name].(json.Number)
		if !ok {
			t.Fatalf("%s: %T, want json.Number", name, probe[name])
		}
		if number.String() != want {
			t.Fatalf("%s = %s, want %s", name, number, want)
		}
	}

	// JSON() is the exact document the library produced; JSON encoding may
	// compact it but must keep every integer literal unchanged.
	if string(decoded.JSON()) != decodedDocument {
		t.Fatalf("Decoded.JSON changed the document:\n%s", decoded.JSON())
	}
	roundTrip, err := json.Marshal(&decoded)
	if err != nil {
		t.Fatal(err)
	}
	var compact bytes.Buffer
	if err := json.Compact(&compact, []byte(decodedDocument)); err != nil {
		t.Fatal(err)
	}
	if string(roundTrip) != compact.String() {
		t.Fatalf("decoded document changed:\n%s\nwant:\n%s", roundTrip, compact.String())
	}

	// The positional signals alone (the encode "signals" form) keep the digits.
	body, err := requestJSON(map[string]any{"signals": decoded.PositionalSignals})
	if err != nil {
		t.Fatal(err)
	}
	for _, digits := range []string{"9007199254740993", "-9223372036854775808", "18446744073709551615"} {
		if !strings.Contains(body, digits) {
			t.Fatalf("encode request lost %s: %s", digits, body)
		}
	}
}

func TestDecodedEditsAreSerialized(t *testing.T) {
	var decoded Decoded
	if err := json.Unmarshal([]byte(decodedDocument), &decoded); err != nil {
		t.Fatal(err)
	}
	decoded.PositionalSignals["Probe"]["numberBits"] = json.Number("18446744073709551614")
	decoded.SiteID = "edited"
	decoded.Metadata.OriginalReport = json.RawMessage(`{"unsigned":18446744073709551615}`)
	for _, value := range []any{&decoded, decoded, document(&decoded)} {
		body, err := json.Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Contains(body, []byte(`"siteId":"edited"`)) ||
			!bytes.Contains(body, []byte(`"numberBits":18446744073709551614`)) ||
			!bytes.Contains(body, []byte(`"originalReport":{"unsigned":18446744073709551615}`)) ||
			!bytes.Contains(body, []byte(`"signed":-9223372036854775808`)) {
			t.Fatalf("current fields or original report lost: %s", body)
		}
	}
	// The original snapshot is separate from edits and cannot be mutated by callers.
	snapshot := decoded.JSON()
	snapshot[0] = '!'
	if string(decoded.JSON()) != decodedDocument {
		t.Fatal("JSON snapshot was mutated")
	}
}

func TestRequestRejectsNUL(t *testing.T) {
	for _, raw := range []any{"{}\x00ignored", []byte("{}\x00ignored"), json.RawMessage("{}\x00ignored")} {
		if _, err := requestJSON(raw); err == nil {
			t.Fatalf("accepted NUL in %T request", raw)
		}
	}
	if _, err := Call("version\x00ignored", nil); err == nil {
		t.Fatal("accepted NUL in command")
	}
	var gen Generator
	if _, err := gen.Call("schema\x00ignored", nil); err == nil {
		t.Fatal("accepted NUL in method")
	}
}
