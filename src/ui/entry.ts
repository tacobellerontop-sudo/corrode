import { createLogger } from "../api/logger";
import type { SettingsUI } from "./settings";

/**
 * Ways into the settings window:
 *  - Keyboard shortcut Ctrl/Cmd + Shift + O (always available).
 *  - A "Corrode" entry added to Discord's User Settings sidebar. This is DOM-based and
 *    best-effort: it clones one of Discord's own sidebar items so it looks native, and
 *    silently does nothing if Discord's markup changes.
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

export function installEntryPoints(ui: SettingsUI): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (!isShortcut(e)) return;
    e.preventDefault();
    e.stopPropagation();
    ui.toggle();
  };
  window.addEventListener("keydown", onKey, true);

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
    observer.disconnect();
    for (const el of document.querySelectorAll(`[${MARK}]`)) el.remove();
  };
}
