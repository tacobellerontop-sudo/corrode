// Corrode Installer: puts Corrode into the official Discord desktop app (Stable, PTB, Canary),
// repairs it after a Discord update, and takes it out again.
//
// Run it with no arguments for a menu, or see -help for the command line.
package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
)

var (
	stdin = bufio.NewReader(os.Stdin)
	yes   bool
)

func ask(prompt string) string {
	fmt.Print(prompt)
	line, _ := stdin.ReadString('\n')
	return strings.TrimSpace(line)
}

func confirm(prompt string, def bool) bool {
	if yes {
		return true
	}
	hint := " [y/N] "
	if def {
		hint = " [Y/n] "
	}
	a := strings.ToLower(ask(prompt + hint))
	if a == "" {
		return def
	}
	return a == "y" || a == "yes"
}

func main() {
	var (
		doInstall   = flag.Bool("install", false, "install Corrode")
		doRepair    = flag.Bool("repair", false, "re-install Corrode (after a Discord update, or if it stopped loading)")
		doUninstall = flag.Bool("uninstall", false, "remove Corrode")
		doList      = flag.Bool("list", false, "list Discord installs and whether Corrode is in them")
		branch      = flag.String("branch", "", "which Discord: stable, ptb, canary, development or all (default: every one found)")
		location    = flag.String("location", "", "path to a Discord install (its folder, resources folder or .app) instead of searching")
		purge       = flag.Bool("purge", false, "with -uninstall: also delete Corrode's settings and themes")
		noStart     = flag.Bool("no-start", false, "don't start Discord again after closing it")
	)
	flag.BoolVar(&yes, "yes", false, "don't ask; close Discord if it's running")
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "Corrode Installer %s\n\nUsage: %s [-install | -repair | -uninstall | -list] [options]\nWith no action, shows a menu.\n\n", versionString(), filepath.Base(os.Args[0]))
		flag.PrintDefaults()
	}
	flag.Parse()

	scripted := *doInstall || *doRepair || *doUninstall || *doList
	if scripted {
		yes = true
		installs, err := pickInstalls(*location, *branch)
		if err != nil {
			fail(err)
		}
		switch {
		case *doList:
			printInstalls(installs)
		case *doUninstall:
			if !runUninstall(installs, *purge, !*noStart) {
				os.Exit(1)
			}
		default:
			if !runInstall(installs, !*noStart) {
				os.Exit(1)
			}
		}
		return
	}
	menu(*location, *branch, *noStart)
}

func versionString() string {
	b := buildInfo()
	if b.Commit != "" {
		return b.Version + " (" + b.Commit + ")"
	}
	return b.Version
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "Error:", err)
	os.Exit(1)
}

func pickInstalls(location, branch string) ([]*Install, error) {
	if location != "" {
		i := installFromPath(location)
		if i == nil {
			return nil, fmt.Errorf("couldn't find Discord's app.asar in %s", location)
		}
		return []*Install{i}, nil
	}
	all := findInstalls()
	if branch == "" || branch == "all" {
		if len(all) == 0 {
			return nil, noDiscordError()
		}
		return all, nil
	}
	if branchByID(branch) == nil {
		return nil, fmt.Errorf("unknown -branch %q (use stable, ptb, canary, development or all)", branch)
	}
	for _, i := range all {
		if i.Branch == branch {
			return []*Install{i}, nil
		}
	}
	return nil, fmt.Errorf("%s isn't installed (or isn't where Discord normally puts it; use -location)", branchByID(branch).name)
}

func noDiscordError() error {
	msg := "couldn't find Discord. If it's installed somewhere unusual, pass its folder with -location"
	if fp := flatpakOrSnap(); len(fp) > 0 {
		msg += "\nFound a Flatpak or Snap Discord (" + strings.Join(fp, ", ") + "); those are read-only and not supported. Use the .deb, .rpm or .tar.gz build"
	}
	return fmt.Errorf("%s", msg)
}

func printInstalls(installs []*Install) {
	for n, i := range installs {
		ver := ""
		if i.Version != "" {
			ver = " " + strings.TrimPrefix(i.Version, "app-")
		}
		fmt.Printf("  %d. %-22s %-26s %s\n", n+1, i.Name+ver, stateOf(i.Resources), i.Resources)
	}
}

// closeIfRunning closes Discord (asking first in the menu). Returns whether it was running.
func closeIfRunning(i *Install) (wasRunning bool, ok bool) {
	if !isRunning(i) {
		return false, true
	}
	if !confirm(i.Name+" is running and has to be closed. Close it now?", true) {
		return true, false
	}
	fmt.Printf("Closing %s…\n", i.Name)
	closeDiscord(i)
	return true, true
}

func runInstall(installs []*Install, restart bool) bool {
	dir, err := dataDir()
	if err != nil {
		fmt.Println("Error:", err)
		return false
	}
	if err := writePayload(dir); err != nil {
		fmt.Println("Error: couldn't write Corrode's files to", dir+":", err)
		return false
	}
	patcher := filepath.Join(dir, "patcher.js")
	ok := true
	for _, i := range installs {
		if stateOf(i.Resources) == OtherMod {
			fmt.Printf("%s: %v\n", i.Name, errOtherMod)
			ok = false
			continue
		}
		was, proceed := closeIfRunning(i)
		if !proceed {
			fmt.Printf("%s: skipped.\n", i.Name)
			ok = false
			continue
		}
		if err := patch(i.Resources, patcher); err != nil {
			fmt.Printf("%s: couldn't install Corrode: %v\n", i.Name, err)
			ok = false
		} else {
			fmt.Printf("%s: Corrode is installed.\n", i.Name)
		}
		if was && restart {
			startDiscord(i)
		}
	}
	if ok {
		fmt.Println("\nDone. Open Discord and look for Corrode in User Settings. Its files, settings and themes are in", dir)
		switch runtime.GOOS {
		case "darwin", "linux":
			fmt.Println("When Discord updates itself, run this installer again and choose Repair.")
		default:
			fmt.Println("Corrode adds itself back when Discord updates. If it ever goes missing, run this again and choose Repair.")
		}
	}
	return ok
}

func runUninstall(installs []*Install, purge, restart bool) bool {
	ok := true
	for _, i := range installs {
		was, proceed := closeIfRunning(i)
		if !proceed {
			fmt.Printf("%s: skipped.\n", i.Name)
			ok = false
			continue
		}
		removed := false
		for _, res := range resourcesToUnpatch(i) {
			r, err := unpatch(res)
			if err != nil {
				fmt.Printf("%s: %v\n", i.Name, err)
				ok = false
			}
			removed = removed || r
		}
		if removed {
			fmt.Printf("%s: Corrode is removed.\n", i.Name)
		} else {
			fmt.Printf("%s: Corrode wasn't installed.\n", i.Name)
		}
		if was && restart {
			startDiscord(i)
		}
	}
	if dir, err := dataDir(); err == nil && exists(dir) {
		if purge {
			if err := os.RemoveAll(dir); err != nil {
				fmt.Println("Couldn't delete", dir+":", err)
				ok = false
			} else {
				fmt.Println("Deleted Corrode's settings and themes.")
			}
		} else {
			fmt.Println("Corrode's settings and themes are still in", dir, "(delete that folder to remove them too).")
		}
	}
	return ok
}

func menu(location, branch string, noStart bool) {
	defer func() {
		if runtime.GOOS == "windows" {
			ask("\nPress Enter to close.")
		}
	}()
	fmt.Printf("Corrode Installer %s\nPuts Corrode into the official Discord app.\n\n", versionString())
	for {
		installs, err := pickInstalls(location, branch)
		if err != nil {
			fmt.Println(err)
			p := ask("\nPaste the path to Discord's folder (or press Enter to quit): ")
			if p == "" {
				return
			}
			location = strings.Trim(p, `"'`)
			continue
		}
		fmt.Println("Discord found:")
		printInstalls(installs)
		fmt.Println("\n  [I] Install Corrode   [R] Repair   [U] Uninstall   [Q] Quit")
		choice := strings.ToLower(ask("> "))
		if choice == "" || choice == "q" {
			return
		}
		if choice != "i" && choice != "r" && choice != "u" {
			continue
		}
		targets := installs
		if len(installs) > 1 {
			a := strings.ToLower(ask(fmt.Sprintf("Which Discord? 1-%d, or A for all [A]: ", len(installs))))
			if n, err := strconv.Atoi(a); err == nil && n >= 1 && n <= len(installs) {
				targets = installs[n-1 : n]
			} else if a != "" && a != "a" {
				fmt.Println("Not a choice.")
				continue
			}
		}
		fmt.Println()
		if choice == "u" {
			runUninstall(targets, confirm("Also delete Corrode's settings and themes?", false), !noStart)
		} else {
			runInstall(targets, !noStart)
		}
		fmt.Println()
	}
}
