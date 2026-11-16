package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCompareVersions(t *testing.T) {
	cases := []struct {
		a, b string
		want int
	}{
		{"app-1.0.9188", "app-1.0.9187", 1},
		{"app-1.0.10", "app-1.0.9", 1},
		{"app-1.0.9", "app-1.0.9", 0},
		{"app-0.9.99", "app-1.0.0", -1},
	}
	for _, c := range cases {
		if got := compareVersions(c.a, c.b); got != c.want {
			t.Errorf("compareVersions(%q, %q) = %d, want %d", c.a, c.b, got, c.want)
		}
	}
}

func mkAsar(t *testing.T, dir string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "app.asar"), []byte("asar"), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestWindowsLayout(t *testing.T) {
	root := filepath.Join(t.TempDir(), "Discord")
	mkAsar(t, filepath.Join(root, "app-1.0.9", "resources"))
	mkAsar(t, filepath.Join(root, "app-1.0.10", "resources"))
	_ = os.MkdirAll(filepath.Join(root, "app-1.0.11"), 0o755) // half-installed update: no resources yet
	in := installFromPath(root)
	if in == nil || in.Version != "app-1.0.10" || !strings.HasSuffix(in.Resources, filepath.Join("app-1.0.10", "resources")) || in.Root != root {
		t.Fatalf("installFromPath(root) = %+v", in)
	}
	if got := len(resourcesToUnpatch(in)); got != 2 {
		t.Fatalf("resourcesToUnpatch = %d folders, want 2", got)
	}
}

func TestPatchUnpatch(t *testing.T) {
	res := t.TempDir()
	mkAsar(t, res)
	if s := stateOf(res); s != NotInstalled {
		t.Fatalf("state = %v", s)
	}
	if err := patch(res, "/data/patcher.js"); err != nil {
		t.Fatal(err)
	}
	if s := stateOf(res); s != Installed {
		t.Fatalf("state after patch = %v", s)
	}
	if p, ours := stubTarget(filepath.Join(res, "app.asar")); !ours || p != "/data/patcher.js" {
		t.Fatalf("stubTarget = %q %v", p, ours)
	}
	if err := patch(res, "/data/patcher.js"); err != nil { // repair
		t.Fatal(err)
	}
	if ok, err := unpatch(res); !ok || err != nil {
		t.Fatal(ok, err)
	}
	if data, _ := os.ReadFile(filepath.Join(res, "app.asar")); string(data) != "asar" {
		t.Fatalf("app.asar after unpatch = %q", data)
	}
}

func TestRefusesOtherMod(t *testing.T) {
	res := t.TempDir()
	_ = os.MkdirAll(filepath.Join(res, "app.asar"), 0o755)
	_ = os.WriteFile(filepath.Join(res, "app.asar", "index.js"), []byte(`require("/vencord/patcher.js")`), 0o644)
	_ = os.WriteFile(filepath.Join(res, "_app.asar"), []byte("asar"), 0o644)
	if s := stateOf(res); s != OtherMod {
		t.Fatalf("state = %v", s)
	}
	if err := patch(res, "/data/patcher.js"); err != errOtherMod {
		t.Fatalf("patch = %v", err)
	}
	if _, err := unpatch(res); err != errOtherMod {
		t.Fatalf("unpatch = %v", err)
	}
}
