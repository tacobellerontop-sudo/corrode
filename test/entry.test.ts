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

/** Just enough of React for the layout nodes: effects run when the test flushes them. */
const effects: (() => any)[] = [];
const fakeReact = {
  901: function (module: any) {
    module.exports = {
      useState: (v: any) => [v, () => {}],
      useEffect: (fn: () => any) => void effects.push(fn),
      useRef: (v: any) => ({ current: v }),
      createElement: (type: any, props: any, ...children: any[]) => ({ type, props, children }),
      Fragment: Symbol("Fragment"),
    };
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

  it("adds one page per tab inside Discord's settings when React is available", () => {
    loadChunk(win, fakeReact);
    loadChunk(win, settingsRoot);
    const unmount = vi.fn();
    const ui = { open: vi.fn(), toggle: vi.fn(), mount: vi.fn(() => unmount) } as any;
    const dispose = entry.installEntryPoints(ui);
    const section = webpack.find((m: any) => m?.key === "$Root").buildLayout()[2];
    expect(section.type).toBe(1);
    const items = section.buildLayout();
    expect(items.map((i: any) => i.useTitle())).toEqual(["Plugins", "Themes", "Quick CSS", "About"]);
    expect(items.every((i: any) => i.type === 2 && i.onClick === undefined)).toBe(true);

    const [panel] = items[1].buildLayout();
    const [category] = panel.buildLayout();
    const [custom] = category.buildLayout();
    expect([panel.type, category.type, custom.type]).toEqual([3, 5, 19]);

    // Render the page the way React would: run the component, attach the ref, flush effects.
    effects.length = 0;
    const el = custom.Component();
    const container = document.createElement("div");
    el.props.ref.current = container;
    const cleanup = effects[0]();
    expect(ui.mount).toHaveBeenCalledWith(container, "themes");
    expect(cleanup).toBe(unmount);
    dispose();
  });
});
