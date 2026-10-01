package idle_test

import (
	"os/exec"
	"strings"
	"testing"
)

func TestConsumersLinkOnlyTheEngineTheyOpen(t *testing.T) {
	for _, name := range []string{"empty", "root", "kv", "sqldb", "jobs", "blobs", "records", "metrics", "all"} {
		t.Run(name, func(t *testing.T) {
			command := exec.CommandContext(t.Context(), "go", "list", "-deps", "./cmd/"+name)
			output, err := command.Output()
			if err != nil {
				t.Fatal(err)
			}
			for _, dependency := range strings.Fields(string(output)) {
				for _, engine := range []string{"kv", "sqldb", "jobs", "blobs", "records", "metrics"} {
					path := "github.com/tinyshed/tinystore/" + engine
					if dependency == path && name != engine && name != "all" {
						t.Errorf("%s unexpectedly links %s", name, engine)
					}
				}
				if name == "empty" && dependency == "github.com/tinyshed/tinystore" {
					t.Fatal("empty consumer links TinyStore")
				}
			}
		})
	}
}
