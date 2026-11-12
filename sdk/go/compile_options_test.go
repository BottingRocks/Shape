package shape

import (
	"encoding/json"
	"testing"
)

func TestCompileArtifactDirectOption(t *testing.T) {
	for _, direct := range []bool{false, true} {
		options := CompileArtifactOptions{
			Session: json.RawMessage(`{}`), SiteID: "example.test", Direct: direct,
		}
		encoded, err := json.Marshal(options)
		if err != nil {
			t.Fatal(err)
		}
		var request map[string]any
		if err := json.Unmarshal(encoded, &request); err != nil {
			t.Fatal(err)
		}
		if direct {
			if request["direct"] != true {
				t.Fatalf("direct opt-in lost: %s", encoded)
			}
		} else if _, present := request["direct"]; present {
			t.Fatalf("default should omit direct: %s", encoded)
		}
	}
}
