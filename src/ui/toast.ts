/**
 * Small toast notifications, rendered in a shadow root so Discord's CSS can't affect them.
 */

export type ToastKind = "info" | "success" | "warning" | "error";
export interface ToastOptions {
  kind?: ToastKind;
  /** Milliseconds; 0 keeps it until clicked. Default 4000. */
  duration?: number;
  title?: string;
  onClick?: () => void;
}

let host: HTMLElement | undefined;
let root: ShadowRoot | undefined;

const CSS = `
:host { all: initial; }
.stack { position: fixed; right: 16px; bottom: 16px; z-index: 2147483647; display: flex; flex-direction: column; gap: 8px; align-items: flex-end; pointer-events: none; font-family: var(--font-primary, "gg sans", "Noto Sans", system-ui, sans-serif); }
.toast { pointer-events: auto; cursor: pointer; max-width: 360px; min-width: 220px; box-sizing: border-box; padding: 10px 14px; border-radius: 8px; background: var(--background-floating, #111214); color: var(--text-normal, #dbdee1); box-shadow: 0 8px 16px rgba(0,0,0,.24); border-left: 4px solid var(--accent); font-size: 14px; line-height: 1.35; animation: in .18s ease-out; }
.toast.out { animation: out .15s ease-in forwards; }
.title { font-weight: 600; margin-bottom: 2px; color: var(--header-primary, #f2f3f5); }
.info { --accent: #e0884b; } .success { --accent: #23a55a; } .warning { --accent: #f0b232; } .error { --accent: #f23f43; }
@keyframes in { from { opacity: 0; transform: translateY(8px); } }
@keyframes out { to { opacity: 0; transform: translateX(16px); } }
`;

function ensureRoot(): ShadowRoot {
  if (root && host?.isConnected) return root;
  host = document.createElement("corrode-toasts");
  root = host.attachShadow({ mode: "open" });
  const style = document.createElement("style");
  style.textContent = CSS;
  const stack = document.createElement("div");
  stack.className = "stack";
  root.append(style, stack);
  document.body.appendChild(host);
  return root;
}

export function showToast(message: string, opts: ToastOptions = {}): () => void {
  if (typeof document === "undefined" || !document.body) {
    console.log("[Corrode toast]", message);
    return () => {};
  }
  const stack = ensureRoot().querySelector(".stack")!;
  const el = document.createElement("div");
  el.className = `toast ${opts.kind ?? "info"}`;
  if (opts.title) {
    const t = document.createElement("div");
    t.className = "title";
    t.textContent = opts.title;
    el.appendChild(t);
  }
  el.appendChild(document.createTextNode(message));
  stack.appendChild(el);

  let closed = false;
  const close = () => {
    if (closed) return;
    closed = true;
    el.classList.add("out");
    setTimeout(() => el.remove(), 160);
  };
  el.addEventListener("click", () => {
    opts.onClick?.();
    close();
  });
  const duration = opts.duration ?? 4000;
  if (duration > 0) setTimeout(close, duration);
  return close;
}

export function removeToasts() {
  host?.remove();
  host = undefined;
  root = undefined;
}
