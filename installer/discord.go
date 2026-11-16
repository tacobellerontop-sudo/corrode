package main

import (
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
)

// Install is one Discord installation.
type Install struct {
	Branch    string // stable, ptb, canary, development, or custom
	Name      string // "Discord", "Discord PTB", ...
	Resources string // the folder holding app.asar
	// Windows only: the folder with Update.exe and the app-x.y.z folders, and the exe name.
	Root    string
	Exe     string
	Version string // app-x.y.z on Windows
}

type branchInfo struct {
	id, name, winDir, winExe, macApp, linuxProc string
	linuxDirs                                   []string
}

var branches = []branchInfo{
	{"stable", "Discord", "Discord", "Discord.exe", "Discord.app", "Discord", []string{"discord", "Discord"}},
	{"ptb", "Discord PTB", "DiscordPTB", "DiscordPTB.exe", "Discord PTB.app", "DiscordPTB", []string{"discord-ptb", "DiscordPTB"}},
	{"canary", "Discord Canary", "DiscordCanary", "DiscordCanary.exe", "Discord Canary.app", "DiscordCanary", []string{"discord-canary", "DiscordCanary"}},
	{"development", "Discord Development", "DiscordDevelopment", "DiscordDevelopment.exe", "Discord Development.app", "DiscordDevelopment", []string{"discord-development", "DiscordDevelopment"}},
}

func branchByID(id string) *branchInfo {
	for i := range branches {
		if branches[i].id == id {
			return &branches[i]
		}
	}
	return nil
}

func isDir(p string) bool {
	st, err := os.Stat(p)
	return err == nil && st.IsDir()
}

func exists(p string) bool {
	_, err := os.Lstat(p)
	return err == nil
}

// hasAsar reports whether dir looks like Discord's resources folder.
func hasAsar(dir string) bool {
	return exists(filepath.Join(dir, "app.asar")) || exists(filepath.Join(dir, "_app.asar"))
}

// compareVersions compares "app-1.0.9188" style versions numerically.
func compareVersions(a, b string) int {
	pa := strings.Split(strings.TrimPrefix(a, "app-"), ".")
	pb := strings.Split(strings.TrimPrefix(b, "app-"), ".")
	for i := 0; i < len(pa) || i < len(pb); i++ {
		var x, y int
		if i < len(pa) {
			x, _ = strconv.Atoi(pa[i])
		}
		if i < len(pb) {
			y, _ = strconv.Atoi(pb[i])
		}
		if x != y {
			if x < y {
				return -1
			}
			return 1
		}
	}
	return 0
}

// latestAppDir returns the newest app-x.y.z folder in a Windows Discord root.
func latestAppDir(root string) string {
	entries, err := os.ReadDir(root)
	if err != nil {
		return ""
	}
	var versions []string
	for _, e := range entries {
		if e.IsDir() && strings.HasPrefix(e.Name(), "app-") && hasAsar(filepath.Join(root, e.Name(), "resources")) {
			versions = append(versions, e.Name())
		}
	}
	if len(versions) == 0 {
		return ""
	}
	sort.Slice(versions, func(i, j int) bool { return compareVersions(versions[i], versions[j]) > 0 })
	return versions[0]
}

// windowsInstallFromRoot builds an Install for a root like %LOCALAPPDATA%\Discord.
func windowsInstallFromRoot(b branchInfo, root string) *Install {
	v := latestAppDir(root)
	if v == "" {
		return nil
	}
	return &Install{Branch: b.id, Name: b.name, Resources: filepath.Join(root, v, "resources"), Root: root, Exe: b.winExe, Version: v}
}

// findInstalls looks for Discord in the usual places for this OS.
func findInstalls() []*Install {
	var out []*Install
	home, _ := userHome()
	for _, b := range branches {
		switch runtime.GOOS {
		case "windows":
			local := os.Getenv("LOCALAPPDATA")
			if local == "" {
				local = filepath.Join(home, "AppData", "Local")
			}
			if in := windowsInstallFromRoot(b, filepath.Join(local, b.winDir)); in != nil {
				out = append(out, in)
			}
		case "darwin":
			for _, apps := range []string{"/Applications", filepath.Join(home, "Applications")} {
				res := filepath.Join(apps, b.macApp, "Contents", "Resources")
				if hasAsar(res) {
					out = append(out, &Install{Branch: b.id, Name: b.name, Resources: res})
					break
				}
			}
		default:
			var dirs []string
			for _, d := range b.linuxDirs {
				dirs = append(dirs,
					filepath.Join("/usr/share", d), filepath.Join("/usr/lib", d), filepath.Join("/usr/lib64", d),
					filepath.Join("/opt", d), filepath.Join(home, ".local", "share", d), filepath.Join(home, d))
			}
			for _, d := range dirs {
				res := filepath.Join(d, "resources")
				if hasAsar(res) {
					out = append(out, &Install{Branch: b.id, Name: b.name, Resources: res})
					break
				}
			}
		}
	}
	return out
}

// flatpakOrSnap reports read-only Discord installs this installer can't patch, for a hint.
func flatpakOrSnap() []string {
	if runtime.GOOS != "linux" {
		return nil
	}
	home, _ := userHome()
	var found []string
	for _, p := range []string{
		"/var/lib/flatpak/app/com.discordapp.Discord", "/var/lib/flatpak/app/com.discordapp.DiscordCanary",
		filepath.Join(home, ".local/share/flatpak/app/com.discordapp.Discord"), "/snap/discord",
	} {
		if exists(p) {
			found = append(found, p)
		}
	}
	return found
}

// installFromPath turns a folder the user typed (Discord's install folder, its resources folder,
// or on macOS the .app) into an Install.
func installFromPath(p string) *Install {
	p = filepath.Clean(p)
	if strings.EqualFold(filepath.Base(p), "app.asar") {
		p = filepath.Dir(p)
	}
	name := "Discord (" + p + ")"
	candidates := []string{p, filepath.Join(p, "resources"), filepath.Join(p, "Contents", "Resources")}
	for _, c := range candidates {
		if hasAsar(c) {
			in := &Install{Branch: "custom", Name: name, Resources: c}
			// A Windows-style root with app-x.y.z folders under it.
			if parent := filepath.Dir(filepath.Dir(c)); strings.HasPrefix(filepath.Base(filepath.Dir(c)), "app-") {
				in.Root, in.Version = parent, filepath.Base(filepath.Dir(c))
			}
			return in
		}
	}
	// A Windows root (…\Discord) itself.
	if v := latestAppDir(p); v != "" {
		return &Install{Branch: "custom", Name: name, Resources: filepath.Join(p, v, "resources"), Root: p, Version: v}
	}
	return nil
}
