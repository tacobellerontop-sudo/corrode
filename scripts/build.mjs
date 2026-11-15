// Builds dist/corrode.js (readable), dist/corrode.min.js and dist/manifest.json.
// Usage: node scripts/build.mjs [--watch]
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const watch = process.argv.includes("--watch");

let commit = "unknown";
try {
  commit = execSync("git rev-parse --short HEAD", { cwd: root, stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
} catch {}

/** Bumped only when the injector contract in docs/INJECTOR.md changes incompatibly. */
const INJECTOR_API_VERSION = 1;

const banner = `/*! Corrode v${pkg.version} (${commit}) | ${pkg.license} | injector api v${INJECTOR_API_VERSION} */`;

/** @type {esbuild.BuildOptions} */
const common = {
  entryPoints: [join(root, "src/index.ts")],
  bundle: true,
  format: "iife",
  platform: "browser",
  target: ["chrome110"],
  legalComments: "none",
  banner: { js: banner },
  define: {
    __CORRODE_VERSION__: JSON.stringify(pkg.version),
    __CORRODE_COMMIT__: JSON.stringify(commit),
  },
  logLevel: "info",
};

const builds = [
  { ...common, outfile: join(dist, "corrode.js"), sourcemap: "inline", minify: false },
  { ...common, outfile: join(dist, "corrode.min.js"), sourcemap: false, minify: true },
];

function writeManifest() {
  const files = {};
  for (const name of ["corrode.js", "corrode.min.js"]) {
    const buf = readFileSync(join(dist, name));
    files[name] = { size: buf.length, sha256: createHash("sha256").update(buf).digest("hex") };
  }
  const manifest = {
    name: "corrode",
    version: pkg.version,
    commit,
    injectorApiVersion: INJECTOR_API_VERSION,
    builtAt: new Date().toISOString(),
    main: "corrode.min.js",
    files,
  };
  writeFileSync(join(dist, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
}

mkdirSync(dist, { recursive: true });

if (watch) {
  const ctxs = await Promise.all(
    builds.map((b) =>
      esbuild.context({
        ...b,
        plugins: [{ name: "manifest", setup: (build) => build.onEnd((r) => !r.errors.length && setTimeout(writeManifest, 50)) }],
      }),
    ),
  );
  await Promise.all(ctxs.map((c) => c.watch()));
  console.log("Watching for changes…");
} else {
  await Promise.all(builds.map((b) => esbuild.build(b)));
  writeManifest();
}
