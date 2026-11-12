package shape

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strconv"
)

// Bytes is an explicit JSON u8 array, never encoding/json's base64 string.
// Nil and empty byte values encode as []; optional fields can use omitempty.
type Bytes []byte

func (b Bytes) MarshalJSON() ([]byte, error) {
	var out bytes.Buffer
	out.WriteByte('[')
	for i, v := range b {
		if i > 0 {
			out.WriteByte(',')
		}
		out.WriteString(strconv.FormatUint(uint64(v), 10))
	}
	out.WriteByte(']')
	return out.Bytes(), nil
}
func (b *Bytes) UnmarshalJSON(data []byte) error {
	if bytes.Equal(bytes.TrimSpace(data), []byte("null")) {
		*b = nil
		return nil
	}
	var array []uint16
	if err := json.Unmarshal(data, &array); err != nil {
		return err
	}
	result := make(Bytes, len(array))
	for i, v := range array {
		if v > 255 {
			return fmt.Errorf("byte out of range: %d", v)
		}
		result[i] = byte(v)
	}
	*b = result
	return nil
}

type bytesRequest struct {
	Bytes Bytes `json:"bytes"`
}

// PackValue is a tagged superpack value. Only the field selected by Kind is
// serialized. NumberBits holds exact IEEE-754 bits, including negative zero.
// Object retains entry order; duplicate keys are rejected by the native codec.
type PackValue struct {
	Kind       PackKind
	Bool       bool
	NumberBits uint64
	Text       string
	Array      []PackValue
	Object     []PackEntry
	Binary     Bytes
	Date       uint64
	Extension  *PackExtension
}
type PackKind string

const (
	PackUndefined       PackKind = "undefined"
	PackNull            PackKind = "null"
	PackBool            PackKind = "bool"
	PackNumber          PackKind = "number"
	PackString          PackKind = "string"
	PackArray           PackKind = "array"
	PackObject          PackKind = "object"
	PackBinary          PackKind = "binary"
	PackDate            PackKind = "date"
	PackMissingSentinel PackKind = "missing_sentinel"
	PackOpaqueExtension PackKind = "extension"
)

type PackEntry struct {
	Key   string
	Value PackValue
}

func (e PackEntry) MarshalJSON() ([]byte, error) { return json.Marshal([2]any{e.Key, e.Value}) }
func (e *PackEntry) UnmarshalJSON(data []byte) error {
	var parts []json.RawMessage
	if err := json.Unmarshal(data, &parts); err != nil {
		return err
	}
	if len(parts) != 2 {
		return fmt.Errorf("pack object entry needs key and value")
	}
	if err := json.Unmarshal(parts[0], &e.Key); err != nil {
		return err
	}
	return json.Unmarshal(parts[1], &e.Value)
}

type PackExtension struct {
	ID      uint32     `json:"id"`
	Payload *PackValue `json:"payload"`
}

func (v PackValue) MarshalJSON() ([]byte, error) {
	var value any
	switch v.Kind {
	case PackUndefined, PackNull, PackMissingSentinel:
		return json.Marshal(struct {
			Kind PackKind `json:"kind"`
		}{v.Kind})
	case PackBool:
		value = v.Bool
	case PackNumber:
		value = fmt.Sprintf("%016x", v.NumberBits)
	case PackString:
		value = v.Text
	case PackArray:
		value = v.Array
		if v.Array == nil {
			value = []PackValue{}
		}
	case PackObject:
		value = v.Object
		if v.Object == nil {
			value = []PackEntry{}
		}
	case PackBinary:
		value = v.Binary
	case PackDate:
		value = v.Date
	case PackOpaqueExtension:
		value = v.Extension
	default:
		return nil, fmt.Errorf("unknown pack kind %q", v.Kind)
	}
	return json.Marshal(struct {
		Kind  PackKind `json:"kind"`
		Value any      `json:"value"`
	}{v.Kind, value})
}
func (v *PackValue) UnmarshalJSON(data []byte) error {
	var wire struct {
		Kind  PackKind        `json:"kind"`
		Value json.RawMessage `json:"value"`
	}
	if err := json.Unmarshal(data, &wire); err != nil {
		return err
	}
	next := PackValue{Kind: wire.Kind}
	var target any
	switch wire.Kind {
	case PackUndefined, PackNull, PackMissingSentinel:
		*v = next
		return nil
	case PackBool:
		target = &next.Bool
	case PackNumber:
		var bits string
		if err := json.Unmarshal(wire.Value, &bits); err != nil {
			return err
		}
		if len(bits) != 16 {
			return fmt.Errorf("pack number requires 16 hexadecimal digits")
		}
		n, err := strconv.ParseUint(bits, 16, 64)
		if err != nil {
			return err
		}
		next.NumberBits = n
		*v = next
		return nil
	case PackString:
		target = &next.Text
	case PackArray:
		target = &next.Array
	case PackObject:
		target = &next.Object
	case PackBinary:
		target = &next.Binary
	case PackDate:
		target = &next.Date
	case PackOpaqueExtension:
		target = &next.Extension
	default:
		return fmt.Errorf("unknown pack kind %q", wire.Kind)
	}
	if err := json.Unmarshal(wire.Value, target); err != nil {
		return err
	}
	*v = next
	return nil
}

type PackLimits struct {
	MaxBytes   *uint64 `json:"max_bytes,omitempty"`
	MaxEntries *uint64 `json:"max_entries,omitempty"`
	MaxDepth   *uint64 `json:"max_depth,omitempty"`
}
type SerializerOptions struct {
	KeysetsToOmit            [][]string  `json:"keysets_to_omit,omitempty"`
	DepthBound               *uint64     `json:"depth_bound,omitempty"`
	MissingSentinelExtension bool        `json:"missing_sentinel_extension,omitempty"`
	OpaqueExtensions         bool        `json:"opaque_extensions,omitempty"`
	Limits                   *PackLimits `json:"limits,omitempty"`
}
type PackDecoded struct {
	Value         PackValue `json:"value"`
	OriginalBytes Bytes     `json:"original_bytes"`
	Canonical     bool      `json:"canonical"`
}

func Pack(value PackValue, options SerializerOptions) ([]byte, error) {
	result, err := operationInto[Bytes]("pack", struct {
		Value   PackValue         `json:"value"`
		Options SerializerOptions `json:"options"`
	}{value, options})
	return []byte(result), err
}
func Unpack(data []byte, options SerializerOptions) (*PackDecoded, error) {
	return operationInto[*PackDecoded]("unpack", struct {
		Bytes   Bytes             `json:"bytes"`
		Options SerializerOptions `json:"options"`
	}{Bytes(data), options})
}
func UnpackCanonical(data []byte, options SerializerOptions) (*PackValue, error) {
	return operationInto[*PackValue]("unpack_canonical", struct {
		Bytes   Bytes             `json:"bytes"`
		Options SerializerOptions `json:"options"`
	}{Bytes(data), options})
}
