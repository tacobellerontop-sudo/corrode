import { getLoadMode, getPatches } from "../api/webpack";
import { getBackend } from "../api/storage";
import type { CorrodeNative } from "../native";
import type { PluginManager } from "../plugins/manager";
import type { PluginStatus, SettingDefinition } from "../plugins/types";
import type { ThemeManager } from "../themes";
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

type TabId = "plugins" | "themes" | "quickcss" | "about";
const TABS: { id: TabId; label: string }[] = [
  { id: "plugins", label: "Plugins" },
  { id: "themes", label: "Themes" },
  { id: "quickcss", label: "Quick CSS" },
  { id: "about", label: "About" },
];

/** The Corrode settings window: a modal rendered in a shadow root, independent of Discord's React. */
export class SettingsUI {
  private host?: HTMLElement;
  private main?: HTMLElement;
  private tab: TabId = "plugins";
  private cleanups: (() => void)[] = [];
  private expanded = new Set<string>();
  private filter = "";
  private lastFocus?: Element | null;

  constructor(private deps: SettingsDeps) {}

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
      TABS.map((t) =>
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
    this.cleanups.push(this.deps.themes.subscribe(() => this.tab === "themes" && this.render()));
    this.render();
    (root.querySelector(`[data-tab="${this.tab}"]`) as HTMLElement | null)?.focus();
  }

  close() {
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

  private switchTab(id: TabId) {
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
    this.main.appendChild(h("button", { class: "close", "aria-label": "Close", onClick: () => this.close() }, "✕"));
    const view = { plugins: () => this.renderPlugins(), themes: () => this.renderThemes(), quickcss: () => this.renderQuickCss(), about: () => this.renderAbout() }[this.tab]();
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
    const shown = all.filter((p) => !q || `${p.definition.name} ${p.definition.description ?? ""} ${p.id}`.toLowerCase().includes(q));
    const restart = all.some((p) => p.restartPending);

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
      h("div", { class: "toolbar" }, urlInput, h("button", { class: "btn", onClick: install }, "Install")),
      shown.length ? h("div", { class: "cards" }, shown.map((p) => this.pluginCard(p))) : h("div", { class: "empty" }, "No plugins match your search."),
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
    const list = themes.list();
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

    return h(
      "section",
      null,
      h("h2", null, "Themes"),
      h("p", { class: "lede" }, "CSS themes, loaded from a URL. BetterDiscord-style .theme.css files work if their CSS doesn't rely on BD-only features."),
      h("div", { class: "toolbar" }, urlInput, h("button", { class: "btn", onClick: add }, "Add")),
      list.length
        ? h(
            "div",
            { class: "cards" },
            list.map((t) =>
              h(
                "div",
                { class: "card" },
                h(
                  "div",
                  { class: "card-head" },
                  h(
                    "div",
                    { class: "card-main" },
                    h("div", { class: "card-title" }, t.name),
                    t.url && h("div", { class: "card-desc" }, t.url),
                    h(
                      "div",
                      { class: "card-actions" },
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
                      h("button", { class: "btn small danger", onClick: () => themes.remove(t.id) }, "Remove"),
                    ),
                  ),
                  toggle(t.enabled, (v) => themes.setEnabled(t.id, v), { label: `Enable ${t.name}` }),
                ),
              ),
            ),
          )
        : h("div", { class: "empty" }, "No themes yet. Paste a theme URL above."),
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
