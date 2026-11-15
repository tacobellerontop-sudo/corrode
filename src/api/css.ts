/**
 * CSS injection. Every injected stylesheet is a `<style data-corrode="...">` element in
 * `<head>`, so removing Corrode leaves no trace behind.
 */

export interface InjectedStyle {
  readonly id: string;
  /** Replace the stylesheet's contents. */
  update(css: string): void;
  remove(): void;
}

const live = new Set<InjectedStyle>();

function container(): HTMLElement {
  return document.head ?? document.documentElement;
}

export function injectCss(css: string, id = "anonymous"): InjectedStyle {
  const el = document.createElement("style");
  el.dataset.corrode = id;
  el.textContent = css;
  // `document.head` may not exist yet when loaded early.
  const attach = () => {
    if (!el.isConnected && live.has(handle)) container().appendChild(el);
  };
  const handle: InjectedStyle = {
    id,
    update(next) {
      el.textContent = next;
    },
    remove() {
      live.delete(handle);
      el.remove();
    },
  };
  live.add(handle);
  if (document.head) attach();
  else document.addEventListener("DOMContentLoaded", attach, { once: true });
  return handle;
}

export function removeAllCss() {
  for (const s of [...live]) s.remove();
}

export interface ScopedCss {
  inject(css: string): InjectedStyle;
  removeAll(): void;
}

export function createScopedCss(owner: string): ScopedCss {
  const mine = new Set<InjectedStyle>();
  return {
    inject(css) {
      const s = injectCss(css, owner);
      const remove = s.remove;
      const tracked: InjectedStyle = {
        id: s.id,
        update: s.update,
        remove() {
          mine.delete(tracked);
          remove();
        },
      };
      mine.add(tracked);
      return tracked;
    },
    removeAll() {
      for (const s of [...mine]) s.remove();
    },
  };
}
