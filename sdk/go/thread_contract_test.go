package shape

import (
	"encoding/json"
	"fmt"
	"reflect"
	"testing"
)

// Mirror the successful envelope and value fields emitted by src/sdk/mod.rs.
// A stopped walk is still a successful SDK call, and can return null code.
func TestThreadResultContract(t *testing.T) {
	for _, tc := range []struct {
		name        string
		code        string
		complete    bool
		diagnostics string
		want        []string
	}{
		{"completed", `"function thread() { return; }"`, true, `[]`, []string{}},
		{"diagnostics", `"function thread() { return; }"`, true, `["register mismatch","stack mismatch"]`, []string{"register mismatch", "stack mismatch"}},
		{"stopped", `null`, false, `[]`, []string{}},
		{"stopped diagnostics", `null`, false, `["register mismatch"]`, []string{"register mismatch"}},
		{"empty code", `""`, true, `[]`, []string{}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			env := fmt.Sprintf(`{"ok":true,"value":{
				"code":%s,"complete":%t,"stop":"%s","steps":1,
				"handlers_traced":1,"undeclared":[],"self_check":%s,
				"source_hash":"0123456789abcdef"
			}}`, tc.code, tc.complete, map[bool]string{true: "return", false: "step limit"}[tc.complete], tc.diagnostics)
			value, err := unwrap([]byte(env))
			if err != nil {
				t.Fatal(err)
			}
			var thread Thread
			if err := decodeValue("trace.thread", value, &thread); err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(thread.SelfCheck, tc.want) {
				t.Fatalf("diagnostics = %#v, want %#v", thread.SelfCheck, tc.want)
			}
			if thread.Complete != tc.complete || thread.Steps != 1 || thread.HandlersTraced != 1 || thread.SourceHash != "0123456789abcdef" {
				t.Fatalf("unexpected thread fields: %+v", thread)
			}
			if tc.code == "null" {
				if thread.Code != nil {
					t.Fatalf("null code = %v, want nil", thread.Code)
				}
			} else {
				var want string
				if err := json.Unmarshal([]byte(tc.code), &want); err != nil {
					t.Fatal(err)
				}
				if thread.Code == nil || *thread.Code != want {
					t.Fatalf("code = %v, want %q", thread.Code, want)
				}
			}
			encoded, err := json.Marshal(thread)
			if err != nil {
				t.Fatal(err)
			}
			var fields map[string]json.RawMessage
			if err := json.Unmarshal(encoded, &fields); err != nil {
				t.Fatal(err)
			}
			if string(fields["code"]) != tc.code || string(fields["self_check"]) != tc.diagnostics {
				t.Fatalf("round trip changed code or diagnostics: %s", encoded)
			}
		})
	}
}
