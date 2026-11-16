package main

import (
	"embed"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/user"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
)

//go:embed payload
var payload embed.FS

// payloadFiles are copied to the data folder on every install and repair.
var payloadFiles = []string{"patcher.js", "preload.js", "corrode.min.js", "manifest.json", "installer.json"}

const stubMarker = "corrode-stub"

// userHome is the home of the person running the installer, even under sudo.
func userHome() (string, error) {
	if os.Geteuid() == 0 {
		if name := os.Getenv("SUDO_USER"); name != "" && name != "root" {
			if u, err := user.Lookup(name); err == nil {
				return u.HomeDir, nil
			}
		}
	}
	return os.UserHomeDir()
}

// dataDir is where Corrode's files, settings and themes live.
func dataDir() (string, error) {
	if d := os.Getenv("CORRODE_DATA_DIR"); d != "" {
		return d, nil
	}
	home, err := userHome()
	if err != nil {
		return "", err
	}
	switch runtime.GOOS {
	case "windows":
		if a := os.Getenv("APPDATA"); a != "" {
			return filepath.Join(a, "Corrode"), nil
		}
		return filepath.Join(home, "AppData", "Roaming", "Corrode"), nil
	case "darwin":
		return filepath.Join(home, "Library", "Application Support", "Corrode"), nil
	default:
		if x := os.Getenv("XDG_CONFIG_HOME"); x != "" && os.Geteuid() != 0 {
			return filepath.Join(x, "Corrode"), nil
		}
		return filepath.Join(home, ".config", "Corrode"), nil
	}
}

// chownToSudoUser gives files written under sudo back to the person who ran sudo, so Discord
// (running as them) can still write its settings.
func chownToSudoUser(root string) {
	if os.Geteuid() != 0 {
		return
	}
	uid, err1 := strconv.Atoi(os.Getenv("SUDO_UID"))
	gid, err2 := strconv.Atoi(os.Getenv("SUDO_GID"))
	if err1 != nil || err2 != nil || uid == 0 {
		return
	}
	_ = filepath.WalkDir(root, func(p string, _ fs.DirEntry, err error) error {
		if err == nil {
			_ = os.Lchown(p, uid, gid)
		}
		return nil
	})
}

// BuildInfo is installer.json.
type BuildInfo struct {
	Version        string `json:"version"`
	Commit         string `json:"commit"`
	RuntimeVersion string `json:"runtimeVersion"`
}

func buildInfo() BuildInfo {
	var b BuildInfo
	if data, err := payload.ReadFile("payload/installer.json"); err == nil {
		_ = json.Unmarshal(data, &b)
	}
	if b.Version == "" {
		b.Version = "dev"
	}
	return b
}

// writePayload puts Corrode's files into the data folder.
func writePayload(dir string) error {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	for _, name := range payloadFiles {
		data, err := payload.ReadFile("payload/" + name)
		if err != nil {
			return fmt.Errorf("this installer is missing %s (it was built without running scripts/build-installer.mjs)", name)
		}
		tmp := filepath.Join(dir, name+".tmp")
		if err := os.WriteFile(tmp, data, 0o644); err != nil {
			return err
		}
		if err := os.Rename(tmp, filepath.Join(dir, name)); err != nil {
			return err
		}
	}
	// A fresh install turns Corrode back on and drops builds downloaded by an older install
	// (the one just written is at least as new).
	_ = os.Remove(filepath.Join(dir, "disabled"))
	_ = os.RemoveAll(filepath.Join(dir, "updates"))
	_ = os.MkdirAll(filepath.Join(dir, "themes"), 0o755)
	chownToSudoUser(dir)
	return nil
}

// stubIndex is the index.js that replaces app.asar. Keep in sync with stubIndex() in runtime/patcher.js.
func stubIndex(patcher string) string {
	quoted, _ := json.Marshal(patcher)
	return `// ` + stubMarker + ` v1: written by the Corrode installer. Run the installer again to remove Corrode.
"use strict";
const path = require("path");
const patcher = ` + string(quoted) + `;
if (require("fs").existsSync(patcher)) {
  require(patcher);
} else {
  // Corrode's files are gone: start Discord without it.
  const { app } = require("electron");
  const asar = path.join(process.resourcesPath, "_app.asar");
  const pkg = require(path.join(asar, "package.json"));
  require.main.filename = path.join(asar, pkg.main);
  if (typeof app.setAppPath === "function") app.setAppPath(asar);
  require(require.main.filename);
}
`
}

const stubPackage = "{\n  \"name\": \"discord\",\n  \"main\": \"index.js\",\n  \"private\": true\n}\n"

// State of one Discord install.
type State int

const (
	NotInstalled State = iota
	Installed
	OtherMod // app.asar is a folder that isn't ours (Vencord and friends use the same trick)
	Broken   // neither app.asar nor _app.asar looks right
)

func (s State) String() string {
	return [...]string{"not installed", "Corrode installed", "another mod is installed", "files missing"}[s]
}

func stubTarget(appAsar string) (string, bool) {
	data, err := os.ReadFile(filepath.Join(appAsar, "index.js"))
	if err != nil || !strings.Contains(string(data), stubMarker) {
		return "", false
	}
	s := string(data)
	i := strings.Index(s, "const patcher = ")
	if i < 0 {
		return "", true
	}
	line := s[i+len("const patcher = "):]
	if j := strings.Index(line, ";\n"); j >= 0 {
		var p string
		if json.Unmarshal([]byte(line[:j]), &p) == nil {
			return p, true
		}
	}
	return "", true
}

func stateOf(resources string) State {
	appAsar := filepath.Join(resources, "app.asar")
	st, err := os.Stat(appAsar)
	switch {
	case err != nil:
		return Broken
	case !st.IsDir():
		return NotInstalled
	}
	if _, ours := stubTarget(appAsar); !ours {
		return OtherMod
	}
	if !exists(filepath.Join(resources, "_app.asar")) {
		return Broken
	}
	return Installed
}

func writeStub(appAsar, patcher string) error {
	if err := os.WriteFile(filepath.Join(appAsar, "package.json"), []byte(stubPackage), 0o644); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(appAsar, "index.js"), []byte(stubIndex(patcher)), 0o644)
}

var errOtherMod = errors.New("another client mod is installed here (app.asar is a folder that isn't Corrode's). Uninstall it first, or use Discord's own installer to repair Discord")

// patch makes resources/app.asar load Corrode. Safe to run again (that's Repair).
func patch(resources, patcher string) error {
	appAsar := filepath.Join(resources, "app.asar")
	orig := filepath.Join(resources, "_app.asar")
	st, err := os.Stat(appAsar)
	switch {
	case err == nil && st.IsDir():
		if _, ours := stubTarget(appAsar); !ours {
			return errOtherMod
		}
		if !exists(orig) {
			return fmt.Errorf("Discord's original app.asar (_app.asar) is missing from %s. Reinstall Discord, then run this again", resources)
		}
		return writeStub(appAsar, patcher)
	case err == nil:
		// A plain Discord. A leftover _app.asar is older than the app.asar Discord just put back.
		if exists(orig) {
			old := filepath.Join(resources, "_app.asar.old")
			_ = os.RemoveAll(old)
			if err := os.Rename(orig, old); err != nil {
				return err
			}
		}
		if err := os.Rename(appAsar, orig); err != nil {
			return explainFsError(err)
		}
		if err := os.Mkdir(appAsar, 0o755); err != nil {
			_ = os.Rename(orig, appAsar)
			return explainFsError(err)
		}
		if err := writeStub(appAsar, patcher); err != nil {
			_ = os.RemoveAll(appAsar)
			_ = os.Rename(orig, appAsar)
			return explainFsError(err)
		}
		return nil
	case errors.Is(err, fs.ErrNotExist) && exists(orig):
		// An earlier run stopped halfway.
		if err := os.Mkdir(appAsar, 0o755); err != nil {
			return explainFsError(err)
		}
		return writeStub(appAsar, patcher)
	default:
		return fmt.Errorf("no app.asar in %s; is this Discord's resources folder?", resources)
	}
}

// unpatch puts Discord's own app.asar back. Returns false if Corrode wasn't installed there.
func unpatch(resources string) (bool, error) {
	appAsar := filepath.Join(resources, "app.asar")
	orig := filepath.Join(resources, "_app.asar")
	if !isDir(appAsar) {
		return false, nil
	}
	if _, ours := stubTarget(appAsar); !ours {
		return false, errOtherMod
	}
	if !exists(orig) {
		return false, fmt.Errorf("Discord's original app.asar (_app.asar) is missing from %s. Reinstall Discord to repair it", resources)
	}
	removing := filepath.Join(resources, "app.asar.corrode-removing")
	_ = os.RemoveAll(removing)
	if err := os.Rename(appAsar, removing); err != nil {
		return false, explainFsError(err)
	}
	if err := os.Rename(orig, appAsar); err != nil {
		_ = os.Rename(removing, appAsar)
		return false, explainFsError(err)
	}
	_ = os.RemoveAll(removing)
	return true, nil
}

func explainFsError(err error) error {
	if errors.Is(err, fs.ErrPermission) {
		switch runtime.GOOS {
		case "linux":
			return fmt.Errorf("%w\nDiscord is installed in a system folder: run the installer with sudo", err)
		case "darwin":
			return fmt.Errorf("%w\nmacOS blocked the change: allow your terminal under System Settings > Privacy & Security > App Management (or Full Disk Access), then try again", err)
		default:
			return fmt.Errorf("%w\nMake sure Discord is fully closed (check the system tray) and try again", err)
		}
	}
	return err
}

// resourcesToUnpatch is every resources folder of an install that may carry Corrode. On Windows
// that's each app-x.y.z folder, since an older one may still be patched.
func resourcesToUnpatch(in *Install) []string {
	if in.Root == "" {
		return []string{in.Resources}
	}
	entries, err := os.ReadDir(in.Root)
	if err != nil {
		return []string{in.Resources}
	}
	var out []string
	for _, e := range entries {
		if e.IsDir() && strings.HasPrefix(e.Name(), "app-") {
			if r := filepath.Join(in.Root, e.Name(), "resources"); hasAsar(r) {
				out = append(out, r)
			}
		}
	}
	return out
}
