package main

import (
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"shape"
)

func read(path string) []byte {
	data, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}
	return data
}
func main() {
	artifact, scope, method, request := os.Args[1], os.Args[2], os.Args[3], os.Args[4]
	// RawMessage never passes integer literals through float64.
	body := json.RawMessage(read(request))
	var value json.RawMessage
	var err error
	environment := os.Environ()
	if scope == "surface" {
		var operations []string
		if err := json.Unmarshal(body, &operations); err != nil {
			panic(err)
		}
		for _, operation := range operations {
			namedFunction(operation)
		}
		value = body
	} else if scope == "named" {
		value, err = invokeNamed(method, body)
	} else if scope == "kernel" {
		kernel, failure := shape.Kernel.Load(read(artifact))
		if failure != nil {
			panic(failure)
		}
		defer kernel.Close()
		var fields struct {
			Source string `json:"source"`
		}
		if failure := json.Unmarshal(body, &fields); failure != nil {
			panic(failure)
		}
		var program *shape.Program
		program, err = kernel.TraceProgram(fields.Source, shape.ProgramOptions{})
		if err == nil {
			value, err = json.Marshal(program)
		}
	} else if scope == "v2" {
		value, err = shape.CallOperation(method, body)
	} else if scope == "loaded" {
		var loaded *shape.LoadedArtifact
		loaded, err = shape.LoadArtifact(read(artifact))
		if err != nil {
			panic(err)
		}
		defer loaded.Close()
		detached, failure := loaded.JSON()
		if failure != nil {
			panic(failure)
		}
		detached[0] = '!'
		value, err = invokeLoaded(loaded, method, body)
		if err == nil && method != "encode" {
			for i := 0; i < 2; i++ {
				again, failure := invokeLoaded(loaded, method, body)
				if failure != nil || string(value) != string(again) {
					panic("loaded operation changed on reuse")
				}
			}
		}
	} else if scope == "generator" {
		var generator *shape.Generator
		generator, err = shape.Load(read(artifact))
		if err != nil {
			panic(err)
		}
		defer generator.Close()
		value, err = generator.Call(method, body)
	} else {
		value, err = shape.Call(method, body)
	}
	if !reflect.DeepEqual(environment, os.Environ()) {
		panic("SDK mutated process environment")
	}
	var result any
	if err != nil {
		failure, ok := err.(*shape.Error)
		if !ok {
			panic(err)
		}
		result = map[string]any{"ok": false, "error": failure}
	} else {
		result = map[string]any{"ok": true, "value": value}
	}
	data, err := json.Marshal(result)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(data))
}
