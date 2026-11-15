import { common } from "../api/common";
import { createLogger } from "../api/logger";
import { after } from "../api/patcher";
import { find, filters, onModule } from "../api/webpack";
import { TABS, type SettingsUI, type TabId } from "./settings";

/**
 * Ways into the settings window:
 *  - Keyboard shortcut Ctrl/Cmd + Shift + O (always available).
 *  - A "Corrode" section in Discord's User Settings sidebar, whose items (Plugins, Themes, Quick
 *    CSS, About) are pages inside Discord's own settings pane. Discord builds that sidebar from a
 *    layout tree whose root has the key "$Root"; we add our section to what its `buildLayout()`
 *    returns (the approach BetterDiscord takes; Vencord and Shelter add the same kind of nodes).
 *  - For Discord builds that still use the old tab-list sidebar, a DOM-based fallback that clones
 *    one of Discord's own sidebar items.
 */

const log = createLogger("UI");
const MARK = "data-corrode-entry";

function isShortcut(e: KeyboardEvent) {
  return (e.ctrlKey || e.metaKey) && e.shiftKey && !e.altKey && e.code === "KeyO";
}

/** Finds the user-settings sidebar tab list, if open. */
function findSidebar(): HTMLElement | null {
  const lists = document.querySelectorAll<HTMLElement>('nav [role="tablist"], [class*="sidebar"] [role="tablist"]');
  for (const list of lists) {
    const tabs = list.querySelectorAll('[role="tab"]');
    // The settings sidebar is long; server/channel tab lists are not.
    if (tabs.length >= 8 && list.querySelector('[class*="header"]')) return list;
  }
  return null;
}

function inject(list: HTMLElement, open: () => void) {
  if (list.querySelector(`[${MARK}]`)) return;
  const tabs = [...list.querySelectorAll<HTMLElement>('[role="tab"]')];
  const template = tabs.find((t) => t.getAttribute("aria-selected") !== "true") ?? tabs[0];
  const headerTemplate = list.querySelector<HTMLElement>('[class*="header"]');
  const separatorTemplate = list.querySelector<HTMLElement>('[class*="separator"]');
  if (!template) return;

  const nodes: HTMLElement[] = [];
  if (headerTemplate) {
    const header = headerTemplate.cloneNode(false) as HTMLElement;
    header.textContent = "Corrode";
    nodes.push(header);
  }
  const item = template.cloneNode(false) as HTMLElement;
  item.textContent = "Corrode Settings";
  item.removeAttribute("id");
  item.removeAttribute("aria-controls");
  item.setAttribute("aria-selected", "false");
  item.className = item.className
    .split(/\s+/)
    .filter((c) => !/selected/i.test(c))
    .join(" ");
  item.tabIndex = -1;
  item.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    open();
  });
  item.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open();
    }
  });
  nodes.push(item);
  if (separatorTemplate) nodes.push(separatorTemplate.cloneNode(false) as HTMLElement);
  for (const n of nodes) n.setAttribute(MARK, "");

  // Place it just before the second header (after the "User Settings" group), else at the top.
  const headers = list.querySelectorAll<HTMLElement>('[class*="header"]');
  const anchor = headers[1] ?? list.firstElementChild;
  for (const n of nodes) list.insertBefore(n, anchor);
}

/** Node types of Discord's settings layout tree; the real values are read from Discord when found. */
const FALLBACK_TYPES = { SECTION: 1, SIDEBAR_ITEM: 2, PANEL: 3, CATEGORY: 5, CUSTOM: 19 };
const SECTION_KEY = "corrode_section";

const ICONS: Record<TabId, string[]> = {
  plugins: ["M12 2 3 7v10l9 5 9-5V7z", "M12 22V12M3 7l9 5 9-5"],
  themes: ["M12 3a9 9 0 1 0 0 18c1.5 0 2-1 2-2s-.5-2 1-2h2a4 4 0 0 0 4-4c0-4.4-4-8-9-8z", "M7.5 12h.01M9.5 7.5h.01M14.5 7.5h.01"],
  quickcss: ["m8 7-5 5 5 5", "m16 7 5 5-5 5", "m13.5 4-3 16"],
  updates: ["M12 3v12", "m7 10 5 5 5-5", "M5 21h14"],
  about: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z", "M12 11v5M12 8h.01"],
};

function icon(tab: TabId) {
  const React = common.React;
  if (!React) return null;
  return React.createElement(
    "svg",
    { width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round", strokeLinejoin: "round" },
    ...ICONS[tab].map((d, i) => React.createElement("path", { key: i, d })),
  );
}

/**
 * Builds the "Corrode" section: one sidebar item per tab, each with a page that hosts the
 * matching Corrode view inside Discord's own settings pane (the node shapes Vencord uses).
 * Without React there is nothing to host a page in, so a single item opens the window instead.
 */
function buildSection(ui: SettingsUI) {
  const types = { ...FALLBACK_TYPES, ...find(filters.byProps("SECTION", "SIDEBAR_ITEM", "PANEL")) };
  const React = common.React;
  const base = { usePredicate: () => true };
  let items: any[];
  if (typeof React?.useRef === "function" && typeof React.useEffect === "function") {
    items = (ui.tabs ?? TABS).map(({ id, label }) => {
      const Page = () => {
        const ref = React.useRef(null);
        React.useEffect(() => (ref.current ? ui.mount(ref.current, id) : undefined), []);
        return React.createElement("div", { ref, "data-corrode-page": id });
      };
      const key = `corrode_${id}`;
      const panel = {
        ...base,
        key: `${key}_panel`,
        type: types.PANEL,
        useTitle: () => label,
        buildLayout: () => [
          {
            ...base,
            key: `${key}_category`,
            type: types.CATEGORY,
            buildLayout: () => [{ ...base, key: `${key}_custom`, type: types.CUSTOM, Component: Page, useSearchTerms: () => ["corrode", label] }],
          },
        ],
      };
      return {
        ...base,
        key,
        type: types.SIDEBAR_ITEM,
        useTitle: () => label,
        useSearchTerms: () => ["corrode", label],
        icon: () => icon(id),
        buildLayout: () => [panel],
      };
    });
  } else {
    items = [
      {
        ...base,
        key: "corrode_settings",
        type: types.SIDEBAR_ITEM,
        useTitle: () => "Corrode Settings",
        useSearchTerms: () => ["corrode"],
        icon: () => null,
        // An item with onClick has no page of its own.
        buildLayout: () => [],
        onClick: () => ui.open(),
      },
    ];
  }
  return { ...base, key: SECTION_KEY, type: types.SECTION, useTitle: () => "Corrode", buildLayout: () => items };
}

/** Adds the Corrode section to Discord's settings layout tree, now or when that module loads. */
function installLayoutEntry(ui: SettingsUI): () => void {
  const isRoot = (m: any) => m?.key === "$Root" && typeof m.buildLayout === "function";
  const unpatches: (() => void)[] = [];
  const seen = new WeakSet<object>();
  const patch = (root: any) => {
    if (seen.has(root)) return;
    seen.add(root);
    try {
      unpatches.push(
        after(
          root,
          "buildLayout",
          (_args, layout) => {
            if (!Array.isArray(layout) || layout.some((s) => s?.key === SECTION_KEY)) return;
            const games = layout.findIndex((s) => s?.key === "games_and_apps_section");
            const result = [...layout];
            result.splice(games === -1 ? result.length : games + 1, 0, buildSection(ui));
            return result;
          },
          "corrode-ui",
        ),
      );
      log.info("Added Corrode to Discord's settings sidebar");
    } catch (err) {
      log.warn("Couldn't patch Discord's settings layout", err);
    }
  };
  for (const root of find(isRoot, { all: true })) patch(root);
  const stop = onModule(isRoot, patch);
  return () => {
    stop();
    for (const u of unpatches) u();
  };
}

export function installEntryPoints(ui: SettingsUI): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (!isShortcut(e)) return;
    e.preventDefault();
    e.stopPropagation();
    ui.toggle();
  };
  window.addEventListener("keydown", onKey, true);

  const removeLayoutEntry = installLayoutEntry(ui);

  let scheduled = false;
  const observer = new MutationObserver(() => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      try {
        const list = findSidebar();
        if (list) inject(list, () => ui.open());
      } catch (err) {
        log.warn("Couldn't add the settings entry", err);
      }
    });
  });
  if (document.body) observer.observe(document.body, { childList: true, subtree: true });

  return () => {
    window.removeEventListener("keydown", onKey, true);
    removeLayoutEntry();
    observer.disconnect();
    for (const el of document.querySelectorAll(`[${MARK}]`)) el.remove();
  };
}
