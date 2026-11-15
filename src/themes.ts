import { injectCss, type InjectedStyle } from "./api/css";
import { createLogger } from "./api/logger";
import { createStore } from "./api/storage";
import { fetchText } from "./plugins/manager";

/**
 * Themes (CSS files, installed from a URL or pasted) and Quick CSS (a scratch stylesheet
 * edited live in the settings UI).
 */

const log = createLogger("Themes");

export interface Theme {
  id: string;
  name: string;
  css: string;
  url?: string;
  enabled: boolean;
}

interface ThemeData {
  themes: Theme[];
  quickCss: string;
}

function parseMeta(css: string): { name?: string } {
  // BetterDiscord-style header: /** @name My Theme */
  const name = /@name\s+([^\n*]+)/.exec(css)?.[1]?.trim();
  return { name };
}

export class ThemeManager {
  private store = createStore<ThemeData>("themes");
  private styles = new Map<string, InjectedStyle>();
  private quick?: InjectedStyle;
  private listeners = new Set<() => void>();
  private started = false;

  list(): Theme[] {
    return this.store.get("themes", []);
  }

  getQuickCss(): string {
    return this.store.get("quickCss", "");
  }

  setQuickCss(css: string) {
    this.store.set("quickCss", css);
    if (this.started) this.applyQuickCss();
  }

  subscribe(cb: () => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  private save(themes: Theme[]) {
    this.store.set("themes", themes);
    if (this.started) this.apply();
    for (const l of [...this.listeners]) l();
  }

  start() {
    this.started = true;
    this.apply();
    this.applyQuickCss();
  }

  stop() {
    this.started = false;
    for (const s of this.styles.values()) s.remove();
    this.styles.clear();
    this.quick?.remove();
    this.quick = undefined;
  }

  private apply() {
    const themes = this.list();
    const wanted = new Set(themes.filter((t) => t.enabled).map((t) => t.id));
    for (const [id, s] of this.styles) {
      if (!wanted.has(id)) {
        s.remove();
        this.styles.delete(id);
      }
    }
    for (const t of themes) {
      if (!t.enabled) continue;
      const s = this.styles.get(t.id);
      if (s) s.update(t.css);
      else this.styles.set(t.id, injectCss(t.css, `theme:${t.id}`));
    }
  }

  private applyQuickCss() {
    const css = this.getQuickCss();
    if (!this.quick) this.quick = injectCss(css, "quickcss");
    else this.quick.update(css);
  }

  add(css: string, opts: { name?: string; url?: string } = {}): Theme {
    const themes = this.list();
    const name = opts.name ?? parseMeta(css).name ?? opts.url?.split("/").pop()?.replace(/\.css.*$/, "") ?? "Untitled theme";
    const existing = opts.url ? themes.find((t) => t.url === opts.url) : undefined;
    if (existing) {
      existing.css = css;
      existing.name = name;
      this.save(themes);
      return existing;
    }
    const theme: Theme = { id: Math.random().toString(36).slice(2, 10), name, css, url: opts.url, enabled: true };
    this.save([...themes, theme]);
    return theme;
  }

  async addFromUrl(url: string): Promise<Theme> {
    return this.add(await fetchText(url), { url });
  }

  async refresh(id: string) {
    const t = this.list().find((x) => x.id === id);
    if (!t?.url) return;
    try {
      this.add(await fetchText(t.url), { url: t.url });
    } catch (err) {
      log.error(`Failed to refresh theme ${t.name}`, err);
      throw err;
    }
  }

  setEnabled(id: string, enabled: boolean) {
    this.save(this.list().map((t) => (t.id === id ? { ...t, enabled } : t)));
  }

  remove(id: string) {
    this.save(this.list().filter((t) => t.id !== id));
  }
}
