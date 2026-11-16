import { getLoadMode, getPatches } from "../api/webpack";
import { getBackend } from "../api/storage";
import type { CorrodeNative, UpdateInfo, UpdateTarget } from "../native";
import type { PluginManager } from "../plugins/manager";
import type { PluginStatus, SettingDefinition } from "../plugins/types";
import { PLUGIN_CATEGORIES } from "../builtins/categories";
import { parseMeta, type Theme, type ThemeManager } from "../themes";
import { newState, readState } from "../themeCreator";
import { ThemeCreatorView, type CreatorSession } from "./themeCreatorView";
import { clear, h, toggle } from "./dom";
import { SETTINGS_CSS } from "./styles";
import { showToast } from "./toast";

export interface SettingsDeps {
  plugins: PluginManager;
  themes: ThemeManager;
  native: CorrodeNative;
  version: string;
  commit: string;
  unload(): void;
}

const PLUGIN_GUIDE_URL = "https://github.com/tacobellerontop-sudo/corrode/blob/main/docs/PLUGINS.md";

export type TabId = "plugins" | "themes" | "quickcss" | "updates" | "about";
export const TABS: { id: TabId; label: string }[] = [
  { id: "plugins", label: "Plugins" },
  { id: "themes", label: "Themes" },
  { id: "quickcss", label: "Quick CSS" },
  { id: "updates", label: "Updates" },
  { id: "about", label: "About" },
];

/**
 * Corrode's settings views, rendered in a shadow root, independent of Discord's React.
 * They are shown either embedded in a container (`mount`, used for the pages inside Discord's
 * User Settings) or as a standalone modal window (`open`, the fallback and keyboard shortcut).
 */
export class SettingsUI {
  private host?: HTMLElement;
  private main?: HTMLElement;
  private tab: TabId = "plugins";
  private cleanups: (() => void)[] = [];
  private expanded = new Set<string>();
  private filter = "";
  private category = "";
  private collapsed = new Set<string>();
  /** The theme creator's draft; kept when leaving the page so it can be picked up again. */
  private creator?: CreatorSession;
  private creatorView?: { view: ThemeCreatorView; el: HTMLElement };
  private confirmDelete?: string;
  private themesVisited = false;
  private pasteOpen = false;
  private pasteDraft = "";
  private lastFocus?: Element | null;
  private embedded = false;
  private updates: { info?: UpdateInfo; checking?: boolean; busy?: "client" | "corrode"; error?: string; restart?: boolean } = {};

  constructor(private deps: SettingsDeps) {}

  /** Tabs available here: "Updates" needs a client that supports update checks. */
  get tabs() {
    return TABS.filter((t) => t.id !== "updates" || !!this.deps.native.updates);
  }

  get isOpen() {
    return !!this.host?.isConnected;
  }

  open(tab?: TabId) {
    if (tab) this.tab = tab;
    if (this.isOpen) return this.render();
    this.lastFocus = document.activeElement;
    this.host = document.createElement("corrode-settings");
    const root = this.host.attachShadow({ mode: "open" });
    const style = h("style", null, SETTINGS_CSS);

    const nav = h(
      "nav",
      { role: "tablist", "aria-orientation": "vertical", "aria-label": "Corrode settings" },
      h("div", { class: "brand" }, h("span", { class: "brand-mark" }), h("div", null, "Corrode", h("br"), h("small", null, `v${this.deps.version}`))),
      this.tabs.map((t) =>
        h("button", { class: "tab", role: "tab", "data-tab": t.id, "aria-selected": String(t.id === this.tab), onClick: () => this.switchTab(t.id) }, t.label),
      ),
      h("div", { class: "nav-spacer" }),
    );
    this.main = h("main", { role: "tabpanel" });
    const modal = h("div", { class: "modal", role: "dialog", "aria-modal": "true", "aria-label": "Corrode settings" }, nav, this.main);
    const backdrop = h("div", { class: "backdrop", onMousedown: (e: MouseEvent) => e.target === backdrop && this.close() }, modal);
    root.append(style, backdrop);

    // Keep Discord's global key handlers from reacting while typing in our inputs.
    backdrop.addEventListener("keydown", (e) => e.stopPropagation());
    backdrop.addEventListener("keypress", (e) => e.stopPropagation());
    backdrop.addEventListener("keyup", (e) => e.stopPropagation());

    // Escape closes even when focus fell back to <body> (e.g. after a re-render).
    const onWindowKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && this.isOpen) {
        e.preventDefault();
        e.stopPropagation();
        this.close();
      }
    };
    window.addEventListener("keydown", onWindowKey, true);
    this.cleanups.push(() => window.removeEventListener("keydown", onWindowKey, true));

    document.body.appendChild(this.host);
    this.cleanups.push(this.deps.plugins.subscribe(() => this.tab === "plugins" && this.render()));
    this.cleanups.push(this.deps.themes.subscribe(() => this.tab === "themes" && !this.creator && this.render()));
    this.render();
    (root.querySelector(`[data-tab="${this.tab}"]`) as HTMLElement | null)?.focus();
  }

  /**
   * Renders one tab inside `container` (no window chrome). Only one view exists at a time:
   * mounting closes whatever was showing before. Returns a function that removes it.
   */
  mount(container: HTMLElement, tab: TabId): () => void {
    this.close();
    this.tab = tab;
    this.embedded = true;
    this.lastFocus = null;
    const host = (this.host = document.createElement("corrode-settings"));
    const root = host.attachShadow({ mode: "open" });
    this.main = h("main", null);
    const wrap = h("div", { class: "backdrop embedded" }, this.main);
    root.append(h("style", null, SETTINGS_CSS), wrap);
    // Keep Discord's key handlers from reacting while typing; Escape still closes Discord's settings.
    for (const type of ["keydown", "keypress", "keyup"] as const) {
      wrap.addEventListener(type, (e) => e.key !== "Escape" && e.stopPropagation());
    }
    container.appendChild(host);
    this.cleanups.push(this.deps.plugins.subscribe(() => this.tab === "plugins" && this.render()));
    this.cleanups.push(this.deps.themes.subscribe(() => this.tab === "themes" && !this.creator && this.render()));
    this.render();
    return () => {
      if (this.host === host) this.close();
    };
  }

  close() {
    this.embedded = false;
    this.dropCreatorView();
    this.themesVisited = false;
    for (const c of this.cleanups.splice(0)) c();
    this.host?.remove();
    this.host = undefined;
    this.main = undefined;
    (this.lastFocus as HTMLElement | null)?.focus?.();
  }

  toggle() {
    if (this.isOpen) this.close();
    else this.open();
  }

  /** Ends the creator's live preview; its draft stays for next time. */
  private dropCreatorView() {
    this.creatorView?.view.dispose();
    this.creatorView = undefined;
  }

  private switchTab(id: TabId) {
    if (id !== "themes") this.dropCreatorView();
    this.tab = id;
    this.render();
  }

  private render() {
    if (!this.main || !this.host?.shadowRoot) return;
    for (const b of this.host.shadowRoot.querySelectorAll(".tab")) {
      b.setAttribute("aria-selected", String((b as HTMLElement).dataset.tab === this.tab));
    }
    // Preserve focus/scroll across re-renders.
    const active = this.host.shadowRoot.activeElement as HTMLElement | null;
    const focusKey = active?.dataset?.key;
    const scroll = this.main.scrollTop;
    clear(this.main);
    if (!this.embedded) this.main.appendChild(h("button", { class: "close", "aria-label": "Close", onClick: () => this.close() }, "✕"));
    const view = { plugins: () => this.renderPlugins(), themes: () => this.renderThemes(), quickcss: () => this.renderQuickCss(), updates: () => this.renderUpdates(), about: () => this.renderAbout() }[this.tab]();
    this.main.appendChild(view);
    this.main.scrollTop = scroll;
    if (focusKey) {
      const el = this.main.querySelector(`[data-key="${CSS.escape(focusKey)}"]`) as HTMLInputElement | null;
      el?.focus();
      if (el && "selectionStart" in el && typeof el.value === "string") {
        try {
          el.setSelectionRange(el.value.length, el.value.length);
        } catch {
          /* not a text input */
        }
      }
    }
  }

  /* -------------------------------- plugins -------------------------------- */

  private renderPlugins() {
    const { plugins } = this.deps;
    const all = plugins.list();
    const q = this.filter.toLowerCase();
    const MINE = "Installed by me";
    // Groups by what plugins do (builtins/categories.ts), then any other group a user plugin names.
    const groupOf = (p: PluginStatus) => p.definition.category || (p.source === "external" ? MINE : "Other");
    const known: string[] = PLUGIN_CATEGORIES.map((c) => c.name);
    const groups = [...known, ...[...new Set(all.map(groupOf))].filter((g) => !known.includes(g)).sort((a, b) => (a === MINE ? 1 : b === MINE ? -1 : a.localeCompare(b)))].filter((g) =>
      all.some((p) => groupOf(p) === g),
    );
    const hasMine = all.some((p) => p.source === "external");
    if (this.category && this.category !== "Enabled" && this.category !== MINE && !groups.includes(this.category)) this.category = "";
    if (this.category === MINE && !hasMine) this.category = "";
    const inCategory = (p: PluginStatus) =>
      !this.category || (this.category === "Enabled" ? p.enabled : this.category === MINE ? p.source === "external" : groupOf(p) === this.category);
    const shown = all.filter((p) => inCategory(p) && (!q || `${p.definition.name} ${p.definition.description ?? ""} ${p.id} ${groupOf(p)}`.toLowerCase().includes(q)));
    const restart = all.some((p) => p.restartPending);

    const chip = (value: string, label: string, count: number) =>
      h(
        "button",
        {
          class: "chip",
          type: "button",
          "aria-pressed": String(this.category === value),
          "data-category": value,
          onClick: () => {
            this.category = value;
            this.render();
          },
        },
        label,
        h("span", { class: "chip-count" }, String(count)),
      );
    const chips = h(
      "div",
      { class: "chips", role: "group", "aria-label": "Plugin groups" },
      chip("", "All", all.length),
      chip("Enabled", "Enabled", all.filter((p) => p.enabled).length),
      groups.map((g) => chip(g, g, all.filter((p) => groupOf(p) === g).length)),
      hasMine && !groups.includes(MINE) && chip(MINE, MINE, all.filter((p) => p.source === "external").length),
    );

    const blurb = (g: string) => PLUGIN_CATEGORIES.find((c) => c.name === g)?.description;
    const sections = groups
      .map((g) => {
        const items = shown.filter((p) => groupOf(p) === g).sort((a, b) => a.definition.name.localeCompare(b.definition.name));
        if (!items.length) return null;
        const collapsed = this.collapsed.has(g) && !q;
        return h(
          "div",
          { class: "group", "data-group": g },
          h(
            "button",
            {
              class: "group-head",
              type: "button",
              "aria-expanded": String(!collapsed),
              onClick: () => {
                if (this.collapsed.has(g)) this.collapsed.delete(g);
                else this.collapsed.add(g);
                this.render();
              },
            },
            h("span", { class: "group-caret" }, collapsed ? "▸" : "▾"),
            h("span", { class: "group-name" }, g),
            h("span", { class: "group-count" }, `${items.filter((p) => p.enabled).length}/${items.length} on`),
            blurb(g) && h("span", { class: "group-blurb" }, blurb(g)),
          ),
          !collapsed && h("div", { class: "cards" }, items.map((p) => this.pluginCard(p))),
        );
      })
      .filter(Boolean);

    const urlInput = h("input", { type: "url", placeholder: "Install plugin from URL (https://…/plugin.js)", "aria-label": "Plugin URL" });
    const install = async () => {
      const url = urlInput.value.trim();
      if (!url) return;
      try {
        const id = await plugins.installFromUrl(url);
        await plugins.enable(id);
        showToast(`Installed ${plugins.getDefinition(id)?.name ?? id}`, { kind: "success" });
      } catch (err) {
        showToast(String((err as Error).message ?? err), { kind: "error", title: "Install failed" });
      }
    };
    urlInput.addEventListener("keydown", (e) => e.key === "Enter" && install());

    // Custom plugins can also be pasted in as code (see docs/PLUGINS.md).
    const installPasted = async () => {
      const source = this.pasteDraft.trim();
      if (!source) return;
      try {
        const id = await plugins.installFromSource(source);
        await plugins.enable(id);
        this.pasteDraft = "";
        this.pasteOpen = false;
        showToast(`Installed ${plugins.getDefinition(id)?.name ?? id}`, { kind: "success" });
      } catch (err) {
        showToast(String((err as Error).message ?? err), { kind: "error", title: "That plugin didn't load" });
      }
      this.render();
    };
    const paste =
      this.pasteOpen &&
      h(
        "div",
        { class: "card", "data-paste": "" },
        h("div", { class: "card-desc" }, "Paste a plugin's code. It runs with full access to Discord and your account, so only paste code you trust."),
        h("textarea", {
          rows: 10,
          spellcheck: "false",
          "aria-label": "Plugin code",
          "data-key": "plugin-paste",
          placeholder: 'module.exports = corrode.definePlugin({\n  id: "myPlugin",\n  name: "My Plugin",\n  start(ctx) { ctx.toast("Hello!"); },\n});',
          value: this.pasteDraft,
          onInput: (e: Event) => (this.pasteDraft = (e.target as HTMLTextAreaElement).value),
        }),
        h("div", { class: "card-actions" }, h("button", { class: "btn small", onClick: installPasted }, "Install pasted plugin")),
      );

    return h(
      "section",
      null,
      h("h2", null, "Plugins"),
      h("p", { class: "lede" }, `${all.filter((p) => p.enabled).length} of ${all.length} enabled`),
      restart && this.restartNotice(),
      h(
        "div",
        { class: "toolbar" },
        h("input", {
          type: "text",
          placeholder: "Search plugins",
          "aria-label": "Search plugins",
          value: this.filter,
          "data-key": "plugin-search",
          onInput: (e: Event) => {
            this.filter = (e.target as HTMLInputElement).value;
            this.render();
          },
        }),
      ),
      chips,
      h(
        "div",
        { class: "toolbar" },
        urlInput,
        h("button", { class: "btn", onClick: install }, "Install"),
        h(
          "button",
          {
            class: "btn secondary",
            "aria-expanded": String(this.pasteOpen),
            onClick: () => {
              this.pasteOpen = !this.pasteOpen;
              this.render();
            },
          },
          "Paste code",
        ),
        h("button", { class: "btn secondary", onClick: () => window.open(PLUGIN_GUIDE_URL, "_blank", "noopener") }, "How to make a plugin"),
      ),
      paste,
      sections.length ? h("div", { class: "groups" }, sections) : h("div", { class: "empty" }, "No plugins match your search."),
    );
  }

  private restartNotice() {
    const { native } = this.deps;
    return h(
      "div",
      { class: "notice" },
      "Some changes need a restart to take effect. ",
      native.relaunch
        ? h("button", { class: "btn small", onClick: () => native.relaunch!() }, "Restart Discord")
        : h("button", { class: "btn small", onClick: () => location.reload() }, "Reload"),
    );
  }

  private pluginCard(p: PluginStatus) {
    const { plugins } = this.deps;
    const def = p.definition;
    const hasSettings = !!def.settings && Object.keys(def.settings).length > 0;
    const open = this.expanded.has(p.id);

    const badges = [
      p.source === "external" && h("span", { class: "badge" }, "External"),
      !!def.patches?.length && h("span", { class: "badge", title: "Patches Discord's code; toggling needs a restart" }, "Restart"),
      p.restartPending && h("span", { class: "badge warn" }, "Restart pending"),
      p.error && h("span", { class: "badge err", title: p.error }, "Error"),
    ];

    const actions = [
      hasSettings &&
        h(
          "button",
          {
            class: "btn small secondary",
            "aria-expanded": String(open),
            onClick: () => {
              if (open) this.expanded.delete(p.id);
              else this.expanded.add(p.id);
              this.render();
            },
          },
          open ? "Hide settings" : "Settings",
        ),
      p.source === "external" &&
        p.url &&
        h(
          "button",
          {
            class: "btn small secondary",
            onClick: async () => {
              try {
                await plugins.update(p.id);
                showToast(`Updated ${def.name}`, { kind: "success" });
              } catch (err) {
                showToast(String((err as Error).message ?? err), { kind: "error", title: "Update failed" });
              }
            },
          },
          "Update",
        ),
      p.source === "external" &&
        h(
          "button",
          {
            class: "btn small danger",
            onClick: () => {
              plugins.uninstall(p.id);
              showToast(`Uninstalled ${def.name}`);
            },
          },
          "Uninstall",
        ),
    ].filter(Boolean);

    return h(
      "div",
      { class: "card", "data-plugin": p.id },
      h(
        "div",
        { class: "card-head" },
        h(
          "div",
          { class: "card-main" },
          h("div", { class: "card-title" }, def.name, ...badges),
          def.description && h("div", { class: "card-desc" }, def.description),
          !!def.authors?.length && h("div", { class: "card-desc" }, `by ${def.authors.join(", ")}`),
          p.error && h("div", { class: "card-desc", style: "color:#f23f43" }, p.error),
          actions.length > 0 && h("div", { class: "card-actions" }, actions),
        ),
        toggle(
          p.enabled,
          async (v) => {
            try {
              await plugins.toggle(p.id, v);
            } catch (err) {
              showToast(String((err as Error).message ?? err), { kind: "error" });
              this.render();
            }
          },
          { disabled: def.required, label: `Enable ${def.name}` },
        ),
      ),
      open && hasSettings && this.pluginSettings(p),
    );
  }

  private pluginSettings(p: PluginStatus) {
    const store = this.deps.plugins.settingsStore(p.id);
    const schema = p.definition.settings!;
    return h(
      "div",
      { class: "settings" },
      Object.entries(schema).map(([key, def]) => this.settingField(`${p.id}.${key}`, def as SettingDefinition, store.get(key, (def as SettingDefinition).default), (v) => store.set(key, v))),
      h(
        "div",
        null,
        h(
          "button",
          {
            class: "btn small secondary",
            onClick: () => {
              store.clear();
              this.render();
            },
          },
          "Reset to defaults",
        ),
      ),
    );
  }

  private settingField(key: string, def: SettingDefinition, value: any, set: (v: any) => void) {
    const id = `f-${key.replace(/[^\w-]/g, "_")}`;
    const hint = [def.description, def.restartRequired ? "Requires restart." : ""].filter(Boolean).join(" ");
    const hintEl = hint ? h("div", { class: "hint" }, hint) : null;

    if (def.type === "boolean") {
      return h("div", { class: "field field-row" }, h("div", { class: "field-text" }, h("label", { id }, def.label), hintEl), toggle(!!value, set, { label: def.label }));
    }

    let input: HTMLElement;
    switch (def.type) {
      case "string":
        input = def.multiline
          ? h("textarea", { id, rows: 4, placeholder: def.placeholder, "data-key": key, onChange: (e: Event) => set((e.target as HTMLTextAreaElement).value) }, String(value ?? ""))
          : h("input", { id, type: "text", value: String(value ?? ""), placeholder: def.placeholder, "data-key": key, onChange: (e: Event) => set((e.target as HTMLInputElement).value) });
        break;
      case "number":
        input = h("input", {
          id,
          type: "number",
          value: String(value ?? ""),
          min: def.min,
          max: def.max,
          step: def.step,
          "data-key": key,
          onChange: (e: Event) => {
            const n = Number((e.target as HTMLInputElement).value);
            if (!Number.isNaN(n)) set(n);
          },
        });
        break;
      case "select":
        input = h(
          "select",
          { id, "data-key": key, onChange: (e: Event) => set((e.target as HTMLSelectElement).value) },
          def.options.map((o) => h("option", { value: o.value, selected: o.value === value }, o.label)),
        );
        break;
    }
    return h("div", { class: "field" }, h("label", { for: id }, def.label), hintEl, input);
  }

  /* -------------------------------- themes -------------------------------- */

  private renderThemes() {
    const { themes } = this.deps;
    if (this.creator) {
      this.creatorView ??= (() => {
        const view = new ThemeCreatorView(themes, this.creator!, () => {
          this.creator = undefined;
          this.creatorView = undefined;
          this.render();
        });
        return { view, el: view.render() };
      })();
      return this.creatorView.el;
    }
    if (!this.themesVisited) {
      this.themesVisited = true;
      void themes.refreshFolder();
    }
    const native = themes.native;
    const urlInput = h("input", { type: "url", placeholder: "Theme URL (https://…/theme.css)", "aria-label": "Theme URL" });
    const add = async () => {
      const url = urlInput.value.trim();
      if (!url) return;
      try {
        const t = await themes.addFromUrl(url);
        showToast(`Added ${t.name}`, { kind: "success" });
      } catch (err) {
        showToast(String((err as Error).message ?? err), { kind: "error", title: "Couldn't add theme" });
      }
    };
    urlInput.addEventListener("keydown", (e) => e.key === "Enter" && add());

    // "Add .css file": copies a file into the themes folder (or saved themes without one).
    const fileInput = h("input", {
      type: "file",
      accept: ".css,text/css",
      multiple: true,
      style: "display:none",
      onChange: async (e: Event) => {
        const input = e.target as HTMLInputElement;
        const files = [...(input.files ?? [])];
        input.value = "";
        for (const f of files) {
          try {
            const css = await f.text();
            if (native) await themes.writeFile(f.name.replace(/[\/:*?"<>|]/g, "_").replace(/(\.css)?$/i, ".css"), css);
            else themes.add(css, { name: parseMeta(css).name ?? f.name.replace(/(\.theme)?\.css$/i, "") });
            showToast(`Added ${f.name}`, { kind: "success" });
          } catch (err) {
            showToast(String((err as Error)?.message ?? err), { kind: "error", title: `Couldn't add ${f.name}` });
          }
        }
      },
    });

    const startCreator = (state = newState(), editing?: { id: string; file?: string }) => {
      this.creator = { state, editing, live: true };
      this.render();
    };

    const card = (t: Theme) => {
      const made = readState(t.css);
      const confirming = this.confirmDelete === t.id;
      const meta = parseMeta(t.css);
      return h(
        "div",
        { class: "card", "data-theme": t.id },
        h(
          "div",
          { class: "card-head" },
          h(
            "div",
            { class: "card-main" },
            h("div", { class: "card-title" }, t.name, made && h("span", { class: "badge" }, "Made here"), meta.background && h("span", { class: "badge" }, "Background")),
            t.file && h("div", { class: "card-desc" }, t.file),
            t.url && h("div", { class: "card-desc" }, t.url),
            meta.author && h("div", { class: "card-desc" }, `by ${meta.author}`),
            h(
              "div",
              { class: "card-actions" },
              made && h("button", { class: "btn small", onClick: () => startCreator(made, { id: t.id, file: t.file }) }, "Edit"),
              t.url &&
                h(
                  "button",
                  {
                    class: "btn small secondary",
                    onClick: () =>
                      themes.refresh(t.id).then(
                        () => showToast(`Refreshed ${t.name}`, { kind: "success" }),
                        (err) => showToast(String(err?.message ?? err), { kind: "error" }),
                      ),
                  },
                  "Refresh",
                ),
              confirming
                ? [
                    h(
                      "button",
                      {
                        class: "btn small danger",
                        onClick: async () => {
                          this.confirmDelete = undefined;
                          try {
                            await themes.remove(t.id);
                          } catch (err) {
                            showToast(String((err as Error)?.message ?? err), { kind: "error" });
                            this.render();
                          }
                        },
                      },
                      t.file ? "Delete the file" : "Remove",
                    ),
                    h("button", { class: "btn small secondary", onClick: () => ((this.confirmDelete = undefined), this.render()) }, "Keep"),
                  ]
                : h("button", { class: "btn small danger", onClick: () => ((this.confirmDelete = t.id), this.render()) }, t.file ? "Delete" : "Remove"),
            ),
          ),
          toggle(t.enabled, (v) => themes.setEnabled(t.id, v), { label: `Enable ${t.name}` }),
        ),
      );
    };

    const folder = themes.listFolder();
    const saved = themes.listSaved();
    return h(
      "section",
      null,
      h("h2", null, "Themes"),
      h("p", { class: "lede" }, "Change how Discord looks. Make your own with the theme creator, drop .css files into your themes folder, or add one from a link. BetterDiscord .theme.css files work if they don't rely on BetterDiscord-only features."),
      h(
        "div",
        { class: "create-card" },
        h("div", { class: "create-art" }, ["#5865f2", "#f0b232", "#2ecc71", "#e0457b"].map((c) => h("span", { style: `background:${c}` }))),
        h("div", { class: "card-main" }, h("div", { class: "card-title" }, "Theme creator"), h("div", { class: "card-desc" }, "Pick colours for each part of Discord, add a background picture or video, preview it and save. No coding needed.")),
        h("button", { class: "btn", "data-action": "create-theme", onClick: () => startCreator() }, "Create a theme"),
      ),
      fileInput,
      native
        ? [
            h("h3", null, "Themes folder"),
            h(
              "p",
              { class: "hint" },
              "Put .css files in this folder and they appear here. Changes to a file show up within a few seconds.",
              themes.folderPath && [" ", h("code", { class: "path" }, themes.folderPath)],
            ),
            h(
              "div",
              { class: "toolbar" },
              native.openFolder && h("button", { class: "btn secondary", onClick: () => native.openFolder!().catch((err) => showToast(String(err?.message ?? err), { kind: "error" })) }, "Open themes folder"),
              h("button", { class: "btn secondary", onClick: () => fileInput.click() }, "Add .css file"),
              h("button", { class: "btn secondary", onClick: () => themes.refreshFolder().then(() => this.render()) }, "Reload"),
            ),
            themes.folderError && h("div", { class: "notice" }, `Couldn't read the themes folder: ${themes.folderError}`),
            folder.length
              ? h("div", { class: "cards" }, folder.map(card))
              : h("div", { class: "empty" }, themes.folderReady ? "No .css files in the themes folder yet." : "Loading…"),
            h("h3", null, "From a link"),
          ]
        : h("h3", null, "Your themes"),
      h("div", { class: "toolbar" }, urlInput, h("button", { class: "btn", onClick: add }, "Add"), !native && h("button", { class: "btn secondary", onClick: () => fileInput.click() }, "Add .css file")),
      saved.length ? h("div", { class: "cards" }, saved.map(card)) : h("div", { class: "empty" }, native ? "No themes from links." : "No themes yet. Create one, add a .css file or paste a theme link above."),
    );
  }

  private renderQuickCss() {
    const { themes } = this.deps;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const area = h("textarea", {
      rows: 22,
      spellcheck: false,
      "aria-label": "Quick CSS",
      placeholder: "/* Your CSS applies live as you type */",
      "data-key": "quickcss",
      onInput: (e: Event) => {
        clearTimeout(timer);
        const v = (e.target as HTMLTextAreaElement).value;
        timer = setTimeout(() => themes.setQuickCss(v), 250);
      },
    });
    area.value = themes.getQuickCss();
    area.addEventListener("keydown", (e) => {
      if (e.key === "Tab") {
        e.preventDefault();
        area.setRangeText("  ", area.selectionStart, area.selectionEnd, "end");
        area.dispatchEvent(new Event("input"));
      }
    });
    return h("section", null, h("h2", null, "Quick CSS"), h("p", { class: "lede" }, "A scratch stylesheet, applied live and saved automatically."), area);
  }

  /* -------------------------------- updates -------------------------------- */

  /** Asks the client what is new. Used by the Updates page and once at startup. */
  async checkForUpdates(): Promise<UpdateInfo | undefined> {
    const api = this.deps.native.updates;
    if (!api || this.updates.checking) return this.updates.info;
    this.updates.checking = true;
    this.updates.error = undefined;
    this.renderIf("updates");
    try {
      this.updates.info = await api.check();
    } catch (err) {
      this.updates.error = String((err as Error)?.message ?? err);
    }
    this.updates.checking = false;
    this.renderIf("updates");
    return this.updates.info;
  }

  private renderIf(tab: TabId) {
    if (this.tab === tab) this.render();
  }

  private async runUpdate(which: "client" | "corrode") {
    const api = this.deps.native.updates;
    const run = which === "client" ? api?.updateClient : api?.updateCorrode;
    if (!run || this.updates.busy) return;
    this.updates.busy = which;
    this.updates.error = undefined;
    this.renderIf("updates");
    try {
      await run.call(api);
      if (which === "corrode") this.updates.restart = true;
    } catch (err) {
      this.updates.error = String((err as Error)?.message ?? err);
    }
    this.updates.busy = undefined;
    this.renderIf("updates");
  }

  private renderUpdates() {
    const { native } = this.deps;
    const u = this.updates;
    if (!u.info && !u.checking && !u.error) void this.checkForUpdates();
    const describe = (t?: UpdateTarget) => [t?.version, t?.commit && `(${t.commit})`].filter(Boolean).join(" ") || "Unknown";
    const card = (which: "client" | "corrode", title: string, canUpdate: boolean, action: string) => {
      const t = u.info?.[which];
      const restart = which === "corrode" && u.restart;
      const status = !t
        ? h("span", { class: "badge" }, u.checking ? "Checking…" : "Not checked")
        : t.error
          ? h("span", { class: "badge err" }, "Check failed")
          : restart
            ? h("span", { class: "badge warn" }, "Restart to finish")
            : t.available
              ? h("span", { class: "badge warn" }, "Update available")
              : h("span", { class: "badge" }, "Up to date");
      return h(
        "div",
        { class: "card", "data-update": which },
        h("div", { class: "card-title" }, title, status),
        h("div", { class: "card-desc" }, `Installed: ${describe(t)}`),
        t?.available && h("div", { class: "card-desc" }, `Latest: ${[t.latestVersion, t.latestCommit && `(${t.latestCommit})`].filter(Boolean).join(" ")}`),
        t?.error && h("div", { class: "card-desc" }, t.error),
        h(
          "div",
          { class: "card-actions" },
          restart
            ? native.relaunch && h("button", { class: "btn small", onClick: () => native.relaunch!() }, "Restart now")
            : t?.available && canUpdate && h("button", { class: "btn small", disabled: !!u.busy, onClick: () => void this.runUpdate(which) }, u.busy === which ? "Downloading…" : action),
        ),
      );
    };
    return h(
      "section",
      null,
      h("h2", null, "Updates"),
      h("p", { class: "lede" }, `Updates for ${native.name ?? "the client"} and for Corrode. Corrode checks once each time it starts.`),
      u.error && h("div", { class: "notice" }, u.error),
      h(
        "div",
        { class: "toolbar" },
        h("button", { class: "btn secondary", disabled: !!u.checking, onClick: () => void this.checkForUpdates() }, u.checking ? "Checking…" : "Check for updates"),
      ),
      h(
        "div",
        { class: "cards" },
        card("corrode", "Corrode", !!native.updates?.updateCorrode, "Update Corrode"),
        card("client", native.name ?? "Client", !!native.updates?.updateClient, `Update ${native.name ?? "client"}`),
      ),
    );
  }

  /* -------------------------------- about -------------------------------- */

  private renderAbout() {
    const { native, version, commit, unload, plugins } = this.deps;
    const unapplied = getPatches().filter((p) => !p.applied && !p.noWarn);
    return h(
      "section",
      null,
      h("h2", null, "About Corrode"),
      h("p", { class: "lede" }, "A small, plugin-based client mod for Discord."),
      h(
        "dl",
        { class: "kv" },
        h("dt", null, "Version"),
        h("dd", null, `${version} (${commit})`),
        h("dt", null, "Client"),
        h("dd", null, native.name ? `${native.name}${native.version ? ` ${native.version}` : ""}` : "No injector bridge (browser/userscript)"),
        h("dt", null, "Load mode"),
        h("dd", null, getLoadMode() === "early" ? "Early (all source patches available)" : "Late (some source patches may not apply)"),
        h("dt", null, "Storage"),
        h("dd", null, getBackend().name),
        h("dt", null, "Plugins running"),
        h("dd", null, String(plugins.list().filter((p) => p.running).length)),
        h("dt", null, "Unapplied patches"),
        h("dd", null, unapplied.length ? unapplied.map((p) => p.owner).join(", ") : "None"),
        h("dt", null, "Shortcut"),
        h("dd", null, h("code", null, "Ctrl/⌘ + Shift + O"), " opens this window"),
      ),
      h("h3", null, "Actions"),
      h(
        "div",
        { class: "toolbar" },
        native.relaunch && h("button", { class: "btn secondary", onClick: () => native.relaunch!() }, "Restart Discord"),
        h("button", { class: "btn secondary", onClick: () => location.reload() }, "Reload"),
        native.openDevTools && h("button", { class: "btn secondary", onClick: () => native.openDevTools!() }, "Open DevTools"),
        h(
          "button",
          {
            class: "btn secondary",
            title: "Stops every plugin and removes Corrode from this window until the next reload",
            onClick: () => {
              this.close();
              unload();
            },
          },
          "Unload until reload",
        ),
        native.eject &&
          h(
            "button",
            {
              class: "btn danger",
              onClick: async () => {
                this.close();
                unload();
                try {
                  await native.eject!();
                } catch (err) {
                  showToast(String((err as Error)?.message ?? err), { kind: "error", title: "Eject failed" });
                }
              },
            },
            "Uninstall Corrode",
          ),
      ),
    );
  }
}
