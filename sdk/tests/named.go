package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"shape"
	"strings"
)

// These references are deliberately bound at compile time. Generic Call cannot
// conceal absent public namespace objects or direct operation functions.
var publicRoots = map[string]any{
	"artifact": shape.Artifact, "kernel": shape.Kernel, "ops": shape.Ops,
	"trace": shape.Trace, "cleanup": shape.CleanupOperations, "naming": shape.Naming,
	"signals": shape.Signals, "session": shape.Session, "seeds": shape.Seeds,
	"headers": shape.HeaderOperations, "payload": shape.PayloadOperations,
	"version": shape.NativeVersion, "deob": shape.Deob, "report": shape.Report,
	"pin": shape.Pin, "pack": shape.Pack, "unpack": shape.Unpack,
	"unpack_canonical": shape.UnpackCanonical,
}

func exported(part string) string {
	if part == "to_json" {
		return "ToJSON"
	}
	if part == "parse_url" {
		return "ParseURL"
	}
	if part == "resolve_url" {
		return "ResolveURL"
	}
	fields := strings.Split(part, "_")
	for i, field := range fields {
		fields[i] = strings.ToUpper(field[:1]) + field[1:]
	}
	return strings.Join(fields, "")
}
func namedFunction(operation string) reflect.Value {
	parts := strings.Split(operation, ".")
	value := reflect.ValueOf(publicRoots[parts[0]])
	for _, part := range parts[1:] {
		if !value.IsValid() {
			break
		}
		name := exported(part)
		method := value.MethodByName(name)
		if method.IsValid() {
			value = method
			continue
		}
		if value.Kind() == reflect.Pointer {
			value = value.Elem()
		}
		if value.Kind() != reflect.Struct {
			value = reflect.Value{}
			break
		}
		value = value.FieldByName(name)
	}
	if !value.IsValid() || value.Kind() != reflect.Func {
		panic(fmt.Sprintf("missing named Go API: %s", operation))
	}
	return value
}
func argumentKeys(operation string) []string {
	switch operation {
	case "version", "signals.catalog", "seeds.sites":
		return nil
	case "artifact.compact_schema", "kernel.load", "kernel.to_json":
		return []string{"artifact"}
	case "signals.describe", "seeds.site":
		return []string{"name"}
	case "ops.classify.seeds":
		return []string{"seeds"}
	case "ops.classify.template":
		return []string{"source", "handler_index"}
	case "trace.corpus.compare", "naming.corpus.compare":
		return []string{"baseline", "current"}
	case "trace.corpus.compare_inline":
		return []string{"outlined", "inline"}
	case "naming.corpus.analyze":
		return []string{"sources"}
	case "seeds.parse", "seeds.parse_kernel", "seeds.parse_launcher":
		return []string{"bytes"}
	case "seeds.parse_url":
		return []string{"input"}
	case "seeds.resolve_url":
		return []string{"reference", "base"}
	case "seeds.token":
		return []string{"reference", "param"}
	case "seeds.client":
		return []string{"$"}
	case "headers.codec", "payload.codec":
		return []string{"profile"}
	case "headers.parse":
		return []string{"text"}
	case "headers.encode", "payload.encode":
		return []string{"profile", "report"}
	case "headers.decode":
		return []string{"profile", "headers", "$"}
	case "payload.decode":
		return []string{"profile", "text", "$"}
	case "headers.encode_bytes":
		return []string{"profile", "payload"}
	case "payload.encode_bytes":
		return []string{"profile", "bytes", "seeds"}
	case "headers.decode_bytes", "headers.decode_finalized":
		return []string{"profile", "headers"}
	case "payload.decode_bytes":
		return []string{"profile", "text"}
	case "headers.encode_finalized":
		return []string{"profile", "text", "context"}
	case "pack":
		return []string{"value", "options"}
	case "unpack", "unpack_canonical":
		return []string{"bytes", "options"}
	default:
		return []string{"source", "$"}
	}
}
func exactDecode(raw []byte, out any) error {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	return decoder.Decode(out)
}
func invokeNamed(operation string, request json.RawMessage) (json.RawMessage, error) {
	fn := namedFunction(operation)
	keys := argumentKeys(operation)
	var body map[string]json.RawMessage
	if err := json.Unmarshal(request, &body); err != nil {
		return nil, err
	}
	args := make([]reflect.Value, fn.Type().NumIn())
	for i := range args {
		if i >= len(keys) {
			panic("named adapter missing argument: " + operation)
		}
		data := body[keys[i]]
		if keys[i] == "$" {
			data = request
		}
		if len(data) == 0 {
			data = []byte("null")
		}
		value := reflect.New(fn.Type().In(i))
		if err := exactDecode(data, value.Interface()); err != nil {
			return nil, err
		}
		args[i] = value.Elem()
	}
	result := fn.Call(args)
	if len(result) != 2 {
		panic("expected value,error named signature: " + operation)
	}
	if !result[1].IsNil() {
		return nil, result[1].Interface().(error)
	}
	if loaded, ok := result[0].Interface().(*shape.LoadedArtifact); ok {
		defer loaded.Close()
		return loaded.JSON()
	}
	if operation == "pack" {
		// The public Go API returns bytes. Serialize the test protocol as u8
		// values, matching Python's bytes adapter rather than base64 JSON.
		return json.Marshal(shape.Bytes(result[0].Interface().([]byte)))
	}
	return json.Marshal(result[0].Interface())
}
func invokeLoaded(loaded *shape.LoadedArtifact, method string, request json.RawMessage) (json.RawMessage, error) {
	var body struct {
		Capture  shape.Capture      `json:"capture"`
		Decoded  *shape.Decoded     `json:"decoded"`
		Session  json.RawMessage    `json:"session"`
		Signal   string             `json:"signal"`
		Verify   bool               `json:"verify"`
		Metadata bool               `json:"metadata"`
		Signals  shape.NamedSignals `json:"signals"`
	}
	if err := exactDecode(request, &body); err != nil {
		return nil, err
	}
	var value any
	var err error
	switch method {
	case "decode":
		value, err = shape.Decode(loaded, body.Capture, shape.DecodeOptions{
			Session: body.Session, Verify: body.Verify, Metadata: body.Metadata})
	case "encode":
		if body.Signals != nil {
			value, err = shape.EncodeSignals(loaded, body.Signals, body.Session)
		} else {
			value, err = shape.Encode(loaded, body.Decoded, body.Session)
		}
	case "explain":
		value, err = shape.Explain(loaded, body.Decoded, body.Signal,
			shape.ExplainOptions{Session: body.Session, Verify: body.Verify})
	case "schema":
		value, err = shape.Schema(loaded)
	case "artifact":
		value, err = loaded.JSON()
	default:
		panic("unmapped loaded method " + method)
	}
	if err != nil {
		return nil, err
	}
	return json.Marshal(value)
}
