import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// @ts-expect-error plain JS helper
import { installRuntime, loadChunk } from "./harness/fake-discord.js";

const win = globalThis as any;

/** A module shaped like Discord's settings layout root. */
const settingsRoot = {
  900: function (module: any, exports: any, require: any) {
    const root = {
      key: "$Root",
      type: 0,
      buildLayout: () => [{ key: "user_section" }, { key: "games_and_apps_section" }, { key: "utility_section" }],
    };
    require.d(exports, { R: () => root });
  },
};

describe("settings sidebar entry", () => {
  let webpack: typeof import("../src/api/webpack");
  let entry: typeof import("../src/ui/entry");
  beforeEach(async () => {
    vi.resetModules();
    delete win.webpackChunkdiscord_app;
    webpack = await import("../src/api/webpack");
    entry = await import("../src/ui/entry");
    vi.spyOn(console, "info").mockImplementation(() => {});
    webpack.init(win);
    installRuntime(win);
  });
  afterEach(() => {
    webpack.teardown();
    vi.restoreAllMocks();
  });

  const keys = (layout: any[]) => layout.map((s) => s.key);

  it.each([["before", true], ["after", false]])("adds a Corrode section when the layout loads %s install", (_n, late) => {
    const ui = { open: vi.fn(), toggle: vi.fn() } as any;
    let dispose: () => void;
    if (late) {
      dispose = entry.installEntryPoints(ui);
      loadChunk(win, settingsRoot);
    } else {
      loadChunk(win, settingsRoot);
      dispose = entry.installEntryPoints(ui);
    }
    const root = webpack.find((m: any) => m?.key === "$Root");
    const layout = root.buildLayout();
    expect(keys(layout)).toEqual(["user_section", "games_and_apps_section", "corrode_section", "utility_section"]);
    const [item] = layout[2].buildLayout();
    expect(item.useTitle()).toBe("Corrode Settings");
    expect(item.buildLayout()).toEqual([]);
    item.onClick();
    expect(ui.open).toHaveBeenCalledTimes(1);

    dispose();
    expect(keys(root.buildLayout())).toEqual(["user_section", "games_and_apps_section", "utility_section"]);
  });
});
