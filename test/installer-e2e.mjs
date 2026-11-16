// End-to-end test of the Corrode Installer against a fake Discord: a real Electron with a
// stand-in Discord app (test/installer/fake-discord-app) packed as its app.asar, laid out like
// Discord on Windows (Discord/app-x.y.z/resources). It installs Corrode, starts the fake Discord,
// checks Corrode and CorrodeNative inside it, then repairs, turns off and uninstalls.
//
// Needs `npm run build`, `node scripts/build-installer.mjs --targets linux/amd64` and
// `npm install --no-save electron@33 @electron/asar@3`. On Linux without a display, it uses xvfb-run.
// Usage: node test/installer-e2e.mjs [path/to/installer]
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(join(root, "package.json"));
const installerBin = process.argv[2] || join(root, "dist/installer/CorrodeInstaller-linux-x64");
const electronBin = process.env.ELECTRON_BIN || require("electron");
const asar = require("@electron/asar");

const tmp = mkdtempSync(join(tmpdir(), "corrode-installer-e2e-"));
const discord = join(tmp, "Discord");
const current = join(discord, "app-1.0.1");
const next = join(discord, "app-1.0.2");
const data = join(tmp, "data");
const env = { ...process.env, CORRODE_DATA_DIR: data };

let failures = 0;
function check(name, ok, detail) {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok || detail === undefined ? "" : `: ${JSON.stringify(detail)}`}`);
  if (!ok) failures++;
}

function installer(...args) {
  const r = spawnSync(installerBin, args, { env, encoding: "utf8" });
  const indent = (text, mark) => text.trimEnd().split("\n").map((l) => `  ${mark} ${l}\n`).join("");
  if (r.stdout.trim()) process.stdout.write(indent(r.stdout, "|"));
  if (r.stderr.trim()) process.stdout.write(indent(r.stderr, "!"));
  return r;
}

function runDiscord() {
  const result = join(tmp, "result.json");
  rmSync(result, { force: true });
  const bin = join(current, "electron");
  const args = [bin, "--no-sandbox", "--disable-gpu"];
  const runEnv = {
    ...env,
    FAKE_DISCORD_HARNESS: join(root, "test/harness"),
    FAKE_DISCORD_RESULT: result,
    CORRODE_TEST_HOST_UPDATE: "1",
    ELECTRON_ENABLE_LOGGING: "1",
  };
  const useXvfb = process.platform === "linux" && !process.env.DISPLAY;
  const r = spawnSync(useXvfb ? "xvfb-run" : args[0], useXvfb ? ["-a", ...args] : args.slice(1), { env: runEnv, encoding: "utf8", timeout: 60000 });
  const out = (r.stdout + r.stderr).split("\n").filter((l) => /Corrode|\[page\]|Error/i.test(l) && !/gpu|bus\.cc|viz|node_bindings|Security Warning/i.test(l));
  if (out.length) console.log(out.slice(0, 20).map((l) => "  > " + l).join("\n"));
  return existsSync(result) ? JSON.parse(readFileSync(result, "utf8")) : { error: `no result (exit ${r.status})` };
}

const isDir = (p) => existsSync(p) && statSync(p).isDirectory();

try {
  // Discord/app-1.0.1: Electron with the fake Discord as its app; app-1.0.2: an "update" Discord
  // downloaded but hasn't restarted into yet.
  cpSync(dirname(electronBin), current, { recursive: true, verbatimSymlinks: true });
  rmSync(join(current, "resources/default_app.asar"), { force: true });
  await asar.createPackage(join(root, "test/installer/fake-discord-app"), join(current, "resources/app.asar"));
  cpSync(join(current, "resources/app.asar"), join(next, "resources/app.asar"));

  let r = runDiscord();
  check("plain fake Discord runs without Corrode", r.discordNative === true && r.corrode === false, r);

  r = installer("-list", "-location", current);
  check("lists the install as not installed", /not installed/.test(r.stdout));

  r = installer("-install", "-location", current);
  check("install exits 0", r.status === 0);
  check("app.asar is now the stub folder", isDir(join(current, "resources/app.asar")) && existsSync(join(current, "resources/_app.asar")));
  check("Corrode's files are in the data folder", ["patcher.js", "preload.js", "corrode.min.js", "manifest.json", "installer.json"].every((f) => existsSync(join(data, f))));

  r = runDiscord();
  check("Discord's own preload still runs", r.discordNative === true, r);
  check("Corrode loads", r.corrode === true, r);
  check("Corrode loads early", r.loadMode === "early", r.loadMode);
  check("CorrodeNative is the installer's", r.nativeName === "Corrode Installer", r.nativeName);
  check("storage works", r.storage === "hello", r.storage);
  check("storage is saved to settings.json", JSON.parse(readFileSync(join(data, "settings.json"), "utf8"))["corrode:test"] === "hello");
  check("themes folder lists a written theme", r.themes?.includes("Test.theme.css"), r.themes);
  check("theme file is on disk", existsSync(join(data, "themes/Test.theme.css")));
  check("theme read works", r.themeRead?.includes("@name Test"), r.themeRead);
  check("themes refuse paths outside the folder", /valid file name|isn't a \.css/.test(r.badName), r.badName);
  check("media round-trips (Blob in, ArrayBuffer out)", r.media === "media/bg.png" && JSON.stringify(r.mediaBytes) === "[1,2,3]", r);
  check("updates bridge present", r.hasUpdates === true);
  check("Discord sees its own app path", r.appPath?.endsWith("_app.asar"), r.appPath);
  check("app name stays discord", r.appName === "discord", r.appName);
  check("a downloaded Discord update gets Corrode on quit", isDir(join(next, "resources/app.asar")) && existsSync(join(next, "resources/_app.asar")));

  r = installer("-repair", "-location", current);
  check("repair exits 0", r.status === 0);
  r = installer("-list", "-location", current);
  check("lists the install as installed", /Corrode installed/.test(r.stdout));

  // "Uninstall Corrode" inside Discord leaves this flag when it can't restore files while running.
  writeFileSync(join(data, "disabled"), "");
  r = runDiscord();
  check("turned off: Discord runs without Corrode", r.discordNative === true && r.corrode === false, r);
  installer("-repair", "-location", current);
  check("repair turns it back on", !existsSync(join(data, "disabled")));

  // A missing data folder must not break Discord.
  const moved = data + "-moved";
  cpSync(data, moved, { recursive: true });
  rmSync(data, { recursive: true });
  r = runDiscord();
  check("data folder deleted: Discord still starts", r.discordNative === true && r.corrode === false, r);
  cpSync(moved, data, { recursive: true });

  r = installer("-uninstall", "-location", current);
  check("uninstall exits 0", r.status === 0);
  check("app.asar is Discord's again", statSync(join(current, "resources/app.asar")).isFile() && !existsSync(join(current, "resources/_app.asar")));
  check("the updated app folder is restored too", statSync(join(next, "resources/app.asar")).isFile() && !existsSync(join(next, "resources/_app.asar")));
  check("settings are kept without -purge", existsSync(join(data, "settings.json")));
  r = runDiscord();
  check("after uninstall Discord runs without Corrode", r.discordNative === true && r.corrode === false, r);

  installer("-install", "-location", current);
  r = installer("-uninstall", "-purge", "-location", current);
  check("uninstall -purge deletes the data folder", r.status === 0 && !existsSync(data));
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

console.log(failures ? `\n${failures} check(s) failed` : "\nAll installer checks passed");
process.exit(failures ? 1 : 0);
