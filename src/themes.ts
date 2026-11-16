import { injectCss, type InjectedStyle } from "./api/css";
import { createLogger } from "./api/logger";
import { createStore } from "./api/storage";
import { getNative } from "./native";
import { fetchText } from "./plugins/manager";
import { guessKind, type BackgroundKind } from "./themeCreator";

/**
 * Themes and Quick CSS.
 *
 * - Saved themes: installed from a URL or made in the theme creator, kept in Corrode's storage.
 * - Folder themes: `.css` files in the client's themes folder (when the client has one,
 *   `CorrodeNative.themes`). The folder is checked every few seconds, so edits to a file show up
 *   live. Folder themes start turned off.
 * - Quick CSS: a scratch stylesheet edited live in the settings UI.
 *
 * A theme can have a background picture or video with a `@background <src>` header line
 * (a URL, a data: URL, or `media/<file>` in the themes folder). Corrode shows it in a
 * `<corrode-theme-bg>` layer behind Discord, which the theme's CSS can style.
 */

const log = createLogger("Themes");
const POLL_MS = 3000;

export interface Theme {
  id: string;
  name: string;
  css: string;
  url?: string;
  enabled: boolean;
  /** Set for folder themes: the file's name. */
  file?: string;
}

interface ThemeData {
  themes: Theme[];
  quickCss: string;
  /** Folder themes that are turned on, by file name. */
  folderEnabled: Record<string, boolean>;
}

export function parseMeta(css: string): { name?: string; author?: string; background?: string } {
  // BetterDiscord-style header: /** @name My Theme */
  const tag = (t: string) => new RegExp(`@${t}\\s+([^\\n*]+)`).exec(css)?.[1]?.trim();
  return { name: tag("name"), author: tag("author"), background: /@background\s+([^\s*]+)/.exec(css)?.[1] };
}

const folderId = (file: string) => `file:${file}`;

export class ThemeManager {
  private store = createStore<ThemeData>("themes");
  private styles = new Map<string, InjectedStyle>();
  private quick?: InjectedStyle;
  private previewStyle?: InjectedStyle;
  private preview?: { css: string; replaces?: string };
  private listeners = new Set<() => void>();
  private started = false;
  private folder = new Map<string, { modified: number; css: string }>();
  private folderLoaded = false;
  private poll?: ReturnType<typeof setInterval>;
  private onVisibility?: () => void;
  private polling?: Promise<void>;
  private bg?: { el: HTMLElement; src: string; kind: BackgroundKind; url: string; revoke?: () => void };
  private bgSeq = 0;
  /** Where the folder is, for showing to the user. */
  folderPath?: string;
  folderError?: string;

  /** The client's themes folder, if it has one. */
  get native() {
    return getNative().themes;
  }

  /** Saved themes, then folder themes. */
  list(): Theme[] {
    return [...this.listSaved(), ...this.listFolder()];
  }

  listSaved(): Theme[] {
    return this.store.get("themes", []);
  }

  listFolder(): Theme[] {
    const on = this.store.get("folderEnabled", {});
    return [...this.folder].map(([file, f]) => ({ id: folderId(file), file, name: parseMeta(f.css).name ?? file.replace(/(\.theme)?\.css$/i, ""), css: f.css, enabled: !!on[file] }));
  }

  get folderReady() {
    return this.folderLoaded;
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

  private notify() {
    for (const l of [...this.listeners]) l();
  }

  private save(themes: Theme[]) {
    this.store.set(
      "themes",
      themes.filter((t) => !t.file),
    );
    this.changed();
  }

  private changed() {
    if (this.started) this.apply();
    this.notify();
  }

  start() {
    this.started = true;
    this.apply();
    this.applyQuickCss();
    if (this.native) {
      void this.refreshFolder();
      // Watch the folder only while Discord is on screen. Minimized or in the tray it would
      // only wake the page up every few seconds for nothing; it catches up when shown again.
      this.onVisibility = () => this.setPolling(document.visibilityState !== "hidden");
      document.addEventListener("visibilitychange", this.onVisibility);
      this.onVisibility();
    }
  }

  private setPolling(on: boolean) {
    if (on && !this.poll) {
      void this.refreshFolder();
      this.poll = setInterval(() => void this.refreshFolder(), POLL_MS);
    } else if (!on && this.poll) {
      clearInterval(this.poll);
      this.poll = undefined;
    }
  }

  stop() {
    this.started = false;
    clearInterval(this.poll);
    this.poll = undefined;
    if (this.onVisibility) document.removeEventListener("visibilitychange", this.onVisibility);
    this.onVisibility = undefined;
    for (const s of this.styles.values()) s.remove();
    this.styles.clear();
    this.quick?.remove();
    this.quick = undefined;
    this.previewStyle?.remove();
    this.previewStyle = undefined;
    this.preview = undefined;
    this.setBackground(undefined);
  }

  /** Reads the themes folder again; files that changed are re-read and re-applied. */
  refreshFolder(): Promise<void> {
    const native = this.native;
    if (!native) return Promise.resolve();
    this.polling ??= (async () => {
      let changed = false;
      try {
        const { dir, files } = await native.list();
        this.folderPath = dir;
        if (this.folderError) changed = true;
        this.folderError = undefined;
        const names = new Set(files.map((f) => f.file));
        for (const file of [...this.folder.keys()]) {
          if (!names.has(file)) {
            this.folder.delete(file);
            changed = true;
          }
        }
        for (const f of files) {
          const known = this.folder.get(f.file);
          if (known && known.modified === f.modified) continue;
          try {
            this.folder.set(f.file, { modified: f.modified, css: await native.read(f.file) });
            changed = true;
          } catch (err) {
            log.warn(`Couldn't read ${f.file}`, err);
          }
        }
      } catch (err) {
        const msg = String((err as Error)?.message ?? err);
        if (msg !== this.folderError) changed = true;
        this.folderError = msg;
      } finally {
        if (!this.folderLoaded) changed = true;
        this.folderLoaded = true;
        this.polling = undefined;
      }
      if (changed) this.changed();
    })();
    return this.polling;
  }

  private apply() {
    const themes = this.list();
    const replaced = this.preview?.replaces;
    const wanted = new Set(themes.filter((t) => t.enabled && t.id !== replaced).map((t) => t.id));
    for (const [id, s] of this.styles) {
      if (!wanted.has(id)) {
        s.remove();
        this.styles.delete(id);
      }
    }
    let background: string | undefined;
    for (const t of themes) {
      if (!wanted.has(t.id)) continue;
      const s = this.styles.get(t.id);
      if (s) s.update(t.css);
      else this.styles.set(t.id, injectCss(t.css, `theme:${t.id}`));
      background = parseMeta(t.css).background ?? background;
    }
    if (this.preview) {
      if (!this.previewStyle) this.previewStyle = injectCss(this.preview.css, "theme-preview");
      else this.previewStyle.update(this.preview.css);
      background = parseMeta(this.preview.css).background;
    } else {
      this.previewStyle?.remove();
      this.previewStyle = undefined;
    }
    this.setBackground(background);
  }

  private applyQuickCss() {
    const css = this.getQuickCss();
    if (!this.quick) this.quick = injectCss(css, "quickcss");
    else this.quick.update(css);
  }

  /**
   * Shows `css` live on top of the enabled themes, for the theme creator. `replaces` hides the
   * theme being edited while previewing. `null` ends the preview.
   */
  setPreview(css: string | null, replaces?: string) {
    this.preview = css == null ? undefined : { css, replaces };
    if (this.started) this.apply();
  }

  /* --------------------------- background layer --------------------------- */

  /** Turns a theme's `@background` into a URL the page can load. */
  async resolveMedia(src: string): Promise<{ url: string; revoke?: () => void }> {
    if (!src.startsWith("media/")) return { url: src };
    const native = this.native;
    if (!native?.readMedia) throw new Error(`${src} is in the themes folder, which this client doesn't have`);
    const data = await native.readMedia(src);
    const type = guessKind(src) === "video" ? `video/${/\.webm$/i.test(src) ? "webm" : "mp4"}` : "";
    const url = URL.createObjectURL(new Blob([data], type ? { type } : {}));
    return { url, revoke: () => URL.revokeObjectURL(url) };
  }

  private setBackground(src: string | undefined) {
    if (src && this.bg?.src === src) return;
    const seq = ++this.bgSeq;
    const old = this.bg;
    this.bg = undefined;
    const drop = () => {
      old?.el.remove();
      old?.revoke?.();
    };
    if (!src) return drop();
    const kind = guessKind(src);
    this.resolveMedia(src).then(
      ({ url, revoke }) => {
        if (seq !== this.bgSeq || !this.started) return revoke?.();
        const el = document.createElement("corrode-theme-bg");
        el.setAttribute("aria-hidden", "true");
        el.style.cssText =
          "position:fixed;inset:0;z-index:-1;pointer-events:none;display:block;overflow:hidden;" +
          "background-position:center;background-size:var(--corrode-bg-size,cover);background-repeat:var(--corrode-bg-repeat,no-repeat);";
        if (kind === "video") {
          const v = document.createElement("video");
          Object.assign(v, { src: url, autoplay: true, loop: true, muted: true, playsInline: true });
          v.setAttribute("muted", "");
          v.style.cssText = "width:100%;height:100%;object-fit:var(--corrode-bg-size,cover);";
          v.addEventListener("error", () => log.warn(`Background video didn't load: ${src.slice(0, 120)}`));
          el.appendChild(v);
          void v.play?.()?.catch?.(() => {});
        } else {
          el.style.backgroundImage = `url("${url.replace(/["\\]/g, (c) => "\\" + c)}")`;
        }
        (document.body ?? document.documentElement).prepend(el);
        drop();
        this.bg = { el, src, kind, url, revoke };
      },
      (err) => {
        if (seq !== this.bgSeq) return;
        drop();
        log.warn("Couldn't load the theme background", err);
      },
    );
  }

  /* -------------------------------- editing ------------------------------- */

  add(css: string, opts: { name?: string; url?: string } = {}): Theme {
    const themes = this.listSaved();
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

  /** Replaces a saved theme's CSS (the theme creator's "Save" on a theme it is editing). */
  updateSaved(id: string, css: string) {
    this.save(this.listSaved().map((t) => (t.id === id ? { ...t, css, name: parseMeta(css).name ?? t.name } : t)));
  }

  /** Writes a theme into the themes folder and turns it on. */
  async writeFile(file: string, css: string, enable = true): Promise<Theme> {
    const native = this.native;
    if (!native) throw new Error("This client has no themes folder");
    await native.write(file, css);
    this.folder.set(file, { modified: -1, css });
    if (enable) this.store.set("folderEnabled", { ...this.store.get("folderEnabled", {}), [file]: true });
    this.changed();
    void this.refreshFolder();
    return this.listFolder().find((t) => t.file === file)!;
  }

  async addFromUrl(url: string): Promise<Theme> {
    return this.add(await fetchText(url), { url });
  }

  async refresh(id: string) {
    const t = this.listSaved().find((x) => x.id === id);
    if (!t?.url) return;
    try {
      this.add(await fetchText(t.url), { url: t.url });
    } catch (err) {
      log.error(`Failed to refresh theme ${t.name}`, err);
      throw err;
    }
  }

  setEnabled(id: string, enabled: boolean) {
    const folder = this.listFolder().find((t) => t.id === id);
    if (folder) {
      this.store.set("folderEnabled", { ...this.store.get("folderEnabled", {}), [folder.file!]: enabled });
      this.changed();
      return;
    }
    this.save(this.listSaved().map((t) => (t.id === id ? { ...t, enabled } : t)));
  }

  /** Removes a saved theme, or deletes a folder theme's file. */
  async remove(id: string) {
    const folder = this.listFolder().find((t) => t.id === id);
    if (folder) {
      await this.native!.delete(folder.file!);
      this.folder.delete(folder.file!);
      const on = { ...this.store.get("folderEnabled", {}) };
      delete on[folder.file!];
      this.store.set("folderEnabled", on);
      this.changed();
      return;
    }
    this.save(this.listSaved().filter((t) => t.id !== id));
  }
}
