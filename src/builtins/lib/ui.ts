/**
 * Page UI helpers for the bigger built-in plugins: a dialog window, navigation inside Discord,
 * listening to Flux stores, and keeping an element of ours mounted next to one of Discord's.
 */
import { filters, find } from "../../api/webpack";
import { h } from "../../ui/dom";
import { store, type Ctx } from "./helpers";

/* -------------------------------- dialog --------------------------------- */

const DIALOG_CSS = `
.corrode-dlg-bg { position: fixed; inset: 0; z-index: 2147483100; display: flex; align-items: center; justify-content: center; background: rgba(0,0,0,.6); }
.corrode-dlg { display: flex; flex-direction: column; width: min(var(--corrode-dlg-w, 720px), calc(100vw - 32px)); max-height: min(80vh, 900px);
  background: var(--modal-background, var(--background-primary, #313338)); color: var(--text-normal, #dbdee1); border-radius: 10px;
  box-shadow: 0 12px 40px rgba(0,0,0,.5); font: 14px/1.4 var(--font-primary, "gg sans", system-ui, sans-serif); overflow: hidden; }
.corrode-dlg-head { display: flex; align-items: center; gap: 8px; padding: 14px 16px 10px; }
.corrode-dlg-title { flex: 1; font-size: 18px; font-weight: 600; color: var(--header-primary, #f2f3f5); }
.corrode-dlg-x { all: unset; cursor: pointer; padding: 2px 8px; border-radius: 4px; font-size: 18px; color: var(--interactive-normal, #b5bac1); }
.corrode-dlg-x:hover { background: var(--background-modifier-hover, rgba(255,255,255,.06)); color: var(--interactive-hover, #fff); }
.corrode-dlg-tools { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 0 16px 10px; }
.corrode-dlg-body { overflow: auto; padding: 0 16px 16px; }
.corrode-dlg input[type=text], .corrode-dlg input:not([type]), .corrode-dlg select { font: inherit; color: inherit; border: 0; outline: 0; border-radius: 6px; padding: 7px 10px;
  background: var(--input-background, var(--background-tertiary, #1e1f22)); min-width: 0; }
.corrode-dlg-btn { all: unset; cursor: pointer; padding: 5px 12px; border-radius: 4px; font-size: 13px; font-weight: 500; color: #fff; background: var(--brand-500, #5865f2); }
.corrode-dlg-btn:hover { filter: brightness(1.1); }
.corrode-dlg-btn.sec { color: var(--text-normal, #dbdee1); background: var(--background-modifier-accent, rgba(255,255,255,.1)); }
.corrode-dlg-btn.danger { background: var(--status-danger, #da373c); }
.corrode-dlg-btn[aria-pressed=true] { background: var(--brand-500, #5865f2); color: #fff; }
.corrode-dlg-empty { padding: 32px 0; text-align: center; color: var(--text-muted, #949ba4); }
.corrode-dlg-muted { color: var(--text-muted, #949ba4); font-size: 12px; }
.corrode-dlg-row { padding: 10px 12px; margin-bottom: 8px; border-radius: 8px; background: var(--background-secondary, #2b2d31); }
.corrode-dlg-row-head { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; margin-bottom: 4px; }
.corrode-dlg-row-head b { color: var(--header-primary, #f2f3f5); }
.corrode-dlg-row-actions { display: flex; gap: 6px; margin-left: auto; }
.corrode-dlg-text { white-space: pre-wrap; word-break: break-word; user-select: text; }
.corrode-dlg-old { white-space: pre-wrap; word-break: break-word; color: var(--text-muted, #949ba4); text-decoration: line-through; user-select: text; }
.corrode-dlg-section { margin: 14px 0 6px; font-size: 12px; font-weight: 700; text-transform: uppercase; letter-spacing: .02em; color: var(--header-secondary, #b5bac1); }
.corrode-dlg a { color: var(--text-link, #00a8fc); }
`;

export interface Dialog {
  root: HTMLElement;
  /** Row under the title for search boxes, tabs and buttons. */
  tools: HTMLElement;
  body: HTMLElement;
  setTitle(text: string): void;
  close(): void;
  readonly open: boolean;
}

const openDialogs = new Map<string, Dialog>();

/**
 * Opens a dialog over Discord. One per plugin: opening another replaces it. Closed by Escape, the
 * close button, a click outside, or the plugin stopping.
 */
export function openDialog(ctx: Ctx, title: string, opts: { width?: number; onClose?: () => void } = {}): Dialog {
  openDialogs.get(ctx.id)?.close();
  const style = ctx.css.inject(DIALOG_CSS);
  const titleEl = h("div", { class: "corrode-dlg-title" }, title);
  const tools = h("div", { class: "corrode-dlg-tools" });
  const body = h("div", { class: "corrode-dlg-body" });
  const box = h(
    "div",
    { class: "corrode-dlg", role: "dialog", "aria-modal": "true", "aria-label": title, style: opts.width ? `--corrode-dlg-w:${opts.width}px` : undefined },
    h("div", { class: "corrode-dlg-head" }, titleEl, h("button", { class: "corrode-dlg-x", type: "button", "aria-label": "Close", onClick: () => dialog.close() }, "✕")),
    tools,
    body,
  );
  const root = h("div", { class: "corrode-dlg-bg", "data-corrode-float": ctx.id }, box);
  root.addEventListener("mousedown", (e) => e.target === root && dialog.close());
  // Keep Discord's shortcuts (and its "type anywhere to focus the chat box") away from our inputs.
  for (const type of ["keydown", "keyup", "keypress"]) {
    root.addEventListener(type, (e) => {
      const ev = e as KeyboardEvent;
      if (ev.key === "Escape" && type === "keydown") dialog.close();
      e.stopPropagation();
    });
  }
  let isOpen = true;
  const dialog: Dialog = {
    root,
    tools,
    body,
    setTitle: (t) => void (titleEl.textContent = t),
    close() {
      if (!isOpen) return;
      isOpen = false;
      root.remove();
      style.remove();
      if (openDialogs.get(ctx.id) === dialog) openDialogs.delete(ctx.id);
      try {
        opts.onClose?.();
      } catch (err) {
        ctx.logger.error("Dialog onClose threw", err);
      }
    },
    get open() {
      return isOpen;
    },
  };
  openDialogs.set(ctx.id, dialog);
  ctx.onStop(() => dialog.close());
  document.body.appendChild(root);
  return dialog;
}

/** A row of toggle buttons that act as tabs. */
export function tabs<T extends string>(items: { value: T; label: string }[], value: T, onChange: (v: T) => void): HTMLElement {
  const wrap = h("div", { style: "display:flex;gap:6px", role: "tablist" });
  const render = (current: T) => {
    wrap.replaceChildren(
      ...items.map((it) =>
        h(
          "button",
          {
            class: "corrode-dlg-btn sec",
            role: "tab",
            "aria-pressed": String(it.value === current),
            "aria-selected": String(it.value === current),
            onClick: () => {
              render(it.value);
              onChange(it.value);
            },
          },
          it.label,
        ),
      ),
    );
  };
  render(value);
  return wrap;
}

/* ------------------------------ navigation ------------------------------- */

let router: ((path: string) => void) | undefined;

/** Opens a path inside Discord (e.g. `/channels/@me/123`) without reloading the page. */
export function navigate(path: string) {
  router ??= find(filters.byCode("transitionTo - Transitioning to"));
  if (typeof router === "function") {
    try {
      router(path);
      return;
    } catch {
      /* fall back below */
    }
  }
  // Discord's router listens for popstate, so pushing a history entry and announcing it works too.
  history.pushState(history.state, "", path);
  dispatchEvent(new PopStateEvent("popstate", { state: history.state }));
}

/** The address of a channel, or of one message in it. */
export function channelPath(channelId: string, guildId?: string | null, messageId?: string): string {
  return `/channels/${guildId || "@me"}/${channelId}${messageId ? `/${messageId}` : ""}`;
}

/* -------------------------------- stores --------------------------------- */

/**
 * Calls `fn` (at most once per animation frame) whenever any of the named Flux stores change.
 * Stores that aren't loaded yet are picked up when they appear. Removed when the plugin stops.
 */
export function onStoresChange(ctx: Ctx, names: string[], fn: () => void) {
  let queued = false;
  const run = () => {
    queued = false;
    if (ctx.signal.aborted) return;
    try {
      fn();
    } catch (err) {
      ctx.logger.error("Store listener threw", err);
    }
  };
  const schedule = () => {
    if (queued) return;
    queued = true;
    (typeof requestAnimationFrame === "function" ? requestAnimationFrame : (cb: () => void) => setTimeout(cb, 16))(run);
  };
  const attached = new Map<string, any>();
  const attach = () => {
    for (const name of names) {
      if (attached.has(name)) continue;
      const s = store(name);
      if (!s?.addChangeListener) continue;
      s.addChangeListener(schedule);
      attached.set(name, s);
    }
    return attached.size === names.length;
  };
  if (!attach()) {
    // Keep looking for a while (stores load lazily), then make do with the ones found.
    let tries = 0;
    const t = setInterval(() => {
      if (attach() || ctx.signal.aborted || ++tries > 30) clearInterval(t);
      schedule();
    }, 2000);
    ctx.onStop(() => clearInterval(t));
  }
  ctx.onStop(() => {
    for (const s of attached.values()) s.removeChangeListener?.(schedule);
  });
  schedule();
}

/* ------------------------------- mounting -------------------------------- */

/**
 * Keeps one element of ours next to the first element matching `anchor`, putting it back whenever
 * Discord re-renders. `where` says where it goes relative to the anchor. Removed when the plugin stops.
 */
export function keepMounted(ctx: Ctx, anchor: string, el: HTMLElement, where: "before" | "after" | "prepend" | "append") {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const place = () => {
    timer = undefined;
    if (ctx.signal.aborted) return;
    const a = document.querySelector(anchor);
    if (!a) {
      if (el.isConnected) el.remove();
      return;
    }
    const placed =
      where === "before" ? el.nextSibling === a : where === "after" ? el.previousSibling === a : where === "prepend" ? el.parentNode === a && a.firstChild === el : el.parentNode === a && a.lastChild === el;
    if (placed) return;
    if (where === "before") a.before(el);
    else if (where === "after") a.after(el);
    else if (where === "prepend") a.prepend(el);
    else a.append(el);
  };
  const observer = new MutationObserver(() => {
    if (timer === undefined) timer = setTimeout(place, 100);
  });
  const begin = () => {
    if (ctx.signal.aborted) return;
    observer.observe(document.body, { childList: true, subtree: true });
    place();
  };
  if (document.body) begin();
  else document.addEventListener("DOMContentLoaded", begin, { once: true });
  ctx.onStop(() => {
    observer.disconnect();
    if (timer !== undefined) clearTimeout(timer);
    el.remove();
  });
}

/* -------------------------------- misc ----------------------------------- */

export const avatarUrl = (user: any, size = 64): string | undefined => {
  if (!user?.id) return undefined;
  if (user.avatar) return `https://cdn.discordapp.com/avatars/${user.id}/${user.avatar}.webp?size=${size}`;
  return `https://cdn.discordapp.com/embed/avatars/${Number((BigInt(user.id) >> 22n) % 6n)}.png`;
};

export const guildIconUrl = (guild: any, size = 96): string | undefined =>
  guild?.icon ? `https://cdn.discordapp.com/icons/${guild.id}/${guild.icon}.${String(guild.icon).startsWith("a_") ? "gif" : "webp"}?size=${size}` : undefined;

/** "GS" for "Gaming Server": the letters Discord shows for a server without an icon. */
export const initials = (name: string): string =>
  String(name ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => [...w][0])
    .join("")
    .slice(0, 4);

export function formatTime(at: number | string | Date): string {
  const d = new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  const today = new Date();
  const sameDay = d.toDateString() === today.toDateString();
  return sameDay ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : d.toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}
