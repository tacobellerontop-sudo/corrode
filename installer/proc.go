package main

import (
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

// processName is what Discord's process is called on this OS, or "" if unknown.
func processName(in *Install) string {
	b := branchByID(in.Branch)
	switch runtime.GOOS {
	case "windows":
		return in.Exe
	case "darwin":
		if b != nil {
			return strings.TrimSuffix(b.macApp, ".app")
		}
	default:
		if b != nil {
			return b.linuxProc
		}
	}
	return ""
}

func isRunning(in *Install) bool {
	name := processName(in)
	if name == "" {
		return false
	}
	if runtime.GOOS == "windows" {
		out, err := exec.Command("tasklist", "/FI", "IMAGENAME eq "+name, "/NH").Output()
		return err == nil && strings.Contains(strings.ToLower(string(out)), strings.ToLower(name))
	}
	return exec.Command("pgrep", "-x", name).Run() == nil
}

func closeDiscord(in *Install) {
	name := processName(in)
	if name == "" {
		return
	}
	if runtime.GOOS == "windows" {
		_ = exec.Command("taskkill", "/F", "/T", "/IM", name).Run()
	} else {
		_ = exec.Command("pkill", "-x", name).Run()
	}
	for i := 0; i < 20 && isRunning(in); i++ {
		time.Sleep(250 * time.Millisecond)
	}
	// Give Windows a moment to let go of the files.
	time.Sleep(time.Second)
}

// startDiscord starts Discord again after the installer closed it. Not on Linux, where the
// installer often runs under sudo and would start Discord as root.
func startDiscord(in *Install) {
	switch runtime.GOOS {
	case "windows":
		if in.Root != "" && in.Exe != "" {
			_ = exec.Command(filepath.Join(in.Root, "Update.exe"), "--processStart", in.Exe).Start()
		}
	case "darwin":
		app := filepath.Dir(filepath.Dir(in.Resources)) // …/Discord.app
		_ = exec.Command("open", app).Start()
	}
}
