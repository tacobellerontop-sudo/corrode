import { common } from "../api/common";
import { createLogger } from "../api/logger";
import { after } from "../api/patcher";
import { find, filters, onModule } from "../api/webpack";
import type { SettingsUI } from "./settings";

/**
 * Ways into the settings window:
 *  - Keyboard shortcut Ctrl/Cmd + Shift + O (always available).
 *  - A "Corrode" section in Discord's User Settings sidebar. Discord builds that sidebar from a
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
const FALLBACK_TYPES = { SECTION: 1, SIDEBAR_ITEM: 2 };
const SECTION_KEY = "corrode_section";

function icon() {
  const React = common.React;
  if (!React) return null;
  return React.createElement(
    "svg",
    { width: 20, height: 20, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round", strokeLinejoin: "round" },
    React.createElement("path", { d: "M12 2 3 7v10l9 5 9-5V7z" }),
    React.createElement("path", { d: "M12 22V12M3 7l9 5 9-5" }),
  );
}

function buildSection(open: () => void) {
  const types = find(filters.byProps("SECTION", "SIDEBAR_ITEM", "PANEL")) ?? FALLBACK_TYPES;
  const item = {
    key: "corrode_settings",
    type: types.SIDEBAR_ITEM,
    useTitle: () => "Corrode Settings",
    useSearchTerms: () => ["corrode", "plugins", "themes", "quick css"],
    usePredicate: () => true,
    icon,
    // An item with onClick has no panel of its own: it opens Corrode's window instead.
    buildLayout: () => [],
    onClick: open,
  };
  return {
    key: SECTION_KEY,
    type: types.SECTION,
    useTitle: () => "Corrode",
    usePredicate: () => true,
    buildLayout: () => [item],
  };
}

/** Adds the Corrode section to Discord's settings layout tree, now or when that module loads. */
function installLayoutEntry(open: () => void): () => void {
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
            result.splice(games === -1 ? result.length : games + 1, 0, buildSection(open));
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

  const removeLayoutEntry = installLayoutEntry(() => ui.open());

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
