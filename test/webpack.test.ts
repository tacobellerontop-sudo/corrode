import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// @ts-expect-error plain JS helper
import { coreModules, installRuntime, loadChunk } from "./harness/fake-discord.js";

type Webpack = typeof import("../src/api/webpack");
let webpack: Webpack;
const win = globalThis as any;

beforeEach(async () => {
  vi.resetModules();
  delete win.webpackChunkdiscord_app;
  webpack = await import("../src/api/webpack");
  vi.spyOn(console, "info").mockImplementation(() => {});
  vi.spyOn(console, "debug").mockImplementation(() => {});
});
afterEach(() => {
  webpack.teardown();
  vi.restoreAllMocks();
});

describe("webpack (early load)", () => {
  it("finds modules and resolves waitFor as modules execute", async () => {
    expect(webpack.init(win)).toBe("early");
    const waiting = webpack.waitFor(webpack.filters.byProps("startTyping"));
    installRuntime(win);
    loadChunk(win, coreModules);
    const typing = await waiting;
    expect(typeof typing.startTyping).toBe("function");
    expect(webpack.findByProps("dispatch", "subscribe")).toBeTruthy();
    expect(webpack.findStore("UserStore")?.getCurrentUser().id).toBe("1");
    expect(webpack.find(webpack.filters.byProps("nope"))).toBeUndefined();
    expect(webpack.find(webpack.filters.byProps("startTyping"), { all: true })).toHaveLength(1);
  });

  it("applies source patches with $self before modules run", async () => {
    webpack.init(win);
    win.corrode = { plugins: { getDefinition: () => ({ suffix: "?!" }) } };
    webpack.addPatch(
      { find: '"Hello, "', replacement: { match: /return\s*"Hello, "\s*\+\s*(\w+)\s*\+\s*"!"/, replace: 'return "Howdy, "+$1+$self.suffix' } },
      "test",
      "globalThis.corrode.plugins.getDefinition('test')",
    );
    installRuntime(win);
    loadChunk(win, coreModules);
    const mod = webpack.find(webpack.filters.byProps("greet"));
    expect(mod.greet("Ada")).toBe("Howdy, Ada?!");
    expect(webpack.getPatches()[0].applied).toBe(true);
    delete win.corrode;
  });

  it("does not recurse through webpack's parent push", () => {
    webpack.init(win);
    installRuntime(win);
    expect(() => loadChunk(win, { 1: function () {} })).not.toThrow();
    // our chunk + the runtime's bookkeeping push leaves exactly one entry
    expect(win.webpackChunkdiscord_app.length).toBe(1);
  });
});

describe("webpack (late load)", () => {
  it("hooks an already-running runtime and patches modules that haven't executed", () => {
    installRuntime(win);
    loadChunk(win, { 100: coreModules[100] });
    loadChunk(win, { 400: coreModules[400] }, false); // downloaded, not executed
    expect(webpack.init(win)).toBe("late");
    expect(webpack.getRequire(win)).toBeTypeOf("function");
    expect(webpack.findByProps("dispatch")).toBeTruthy();

    webpack.addPatch({ find: '"Hello, "', replacement: { match: '"Hello, "', replace: '"Hi, "' } }, "test", "null");
    const greet = webpack.getRequire(win)!(400).greet;
    expect(greet("Bob")).toBe("Hi, Bob!");
  });

  it("lazy() resolves on first use", () => {
    installRuntime(win);
    webpack.init(win);
    const Typing = webpack.lazy<any>(webpack.filters.byProps("startTyping"));
    loadChunk(win, { 200: coreModules[200] });
    Typing.startTyping("c1");
    expect(Typing.calls).toEqual(["c1"]);
  });

  it("waitFor times out", async () => {
    installRuntime(win);
    webpack.init(win);
    await expect(webpack.waitFor(webpack.filters.byProps("never"), { timeout: 10 })).rejects.toThrow(/timed out/);
  });

  it("teardown restores the chunk array's push", () => {
    installRuntime(win);
    const before = win.webpackChunkdiscord_app.push;
    webpack.init(win);
    expect(win.webpackChunkdiscord_app.push).not.toBe(before);
    webpack.teardown();
    expect(win.webpackChunkdiscord_app.push).toBe(before);
  });
});
