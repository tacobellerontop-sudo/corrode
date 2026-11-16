// Builds the Corrode Installer (installer/), which puts Corrode into the official Discord app.
// Run `npm run build` first: the installer embeds dist/corrode.min.js and dist/manifest.json.
//
// Usage: node scripts/build-installer.mjs [--targets windows/amd64,darwin/arm64,...]
// Writes dist/installer/<binary> for each target and dist/installer.json (published so the
// in-Discord Updates page can tell when a new installer is needed). Needs Go 1.22+.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const installer = join(root, "installer");
const payload = join(installer, "payload");

const ALL_TARGETS = {
  "windows/amd64": "CorrodeInstaller.exe",
  "windows/arm64": "CorrodeInstaller-arm64.exe",
  "darwin/arm64": "CorrodeInstaller-macos-arm64",
  "darwin/amd64": "CorrodeInstaller-macos-x64",
  "linux/amd64": "CorrodeInstaller-linux-x64",
  "linux/arm64": "CorrodeInstaller-linux-arm64",
};

const arg = process.argv.indexOf("--targets");
const targets = arg > 0 ? process.argv[arg + 1].split(",") : Object.keys(ALL_TARGETS);
for (const t of targets) if (!ALL_TARGETS[t]) throw new Error(`Unknown target ${t}`);

const manifest = JSON.parse(readFileSync(join(dist, "manifest.json"), "utf8"));

// The payload: the runtime that goes into Discord, plus Corrode's build.
for (const f of readdirSync(payload)) if (f !== "README.txt") rmSync(join(payload, f), { recursive: true, force: true });
const runtimeHash = createHash("sha256");
for (const f of ["patcher.js", "preload.js"]) {
  const buf = readFileSync(join(installer, "runtime", f));
  runtimeHash.update(f).update(buf);
  copyFileSync(join(installer, "runtime", f), join(payload, f));
}
copyFileSync(join(dist, manifest.main), join(payload, "corrode.min.js"));
copyFileSync(join(dist, "manifest.json"), join(payload, "manifest.json"));
const info = {
  version: manifest.version,
  commit: manifest.commit,
  // Changes only when the files the installer puts into Discord change.
  runtimeVersion: runtimeHash.digest("hex").slice(0, 16),
  builtAt: manifest.builtAt,
};
writeFileSync(join(payload, "installer.json"), JSON.stringify(info, null, 2) + "\n");

const out = join(dist, "installer");
mkdirSync(out, { recursive: true });
for (const t of targets) {
  const [GOOS, GOARCH] = t.split("/");
  const file = join(out, ALL_TARGETS[t]);
  execFileSync("go", ["build", "-trimpath", "-ldflags", "-s -w", "-o", file, "."], {
    cwd: installer,
    stdio: "inherit",
    env: { ...process.env, GOOS, GOARCH, CGO_ENABLED: "0" },
  });
  console.log(`Built ${file}`);
}
writeFileSync(join(dist, "installer.json"), JSON.stringify(info, null, 2) + "\n");
