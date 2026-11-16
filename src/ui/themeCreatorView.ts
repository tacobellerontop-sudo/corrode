import type { ThemeManager } from "../themes";
import {
  buildCss,
  cleanSrc,
  FONTS,
  guessKind,
  HEX,
  PARTS,
  PRESETS,
  rgba,
  fileName,
  type Background,
  type CreatorState,
  type PartId,
} from "../themeCreator";
import { h } from "./dom";
import { showToast } from "./toast";

/** Pictures and videos picked without a themes folder are put into the theme itself, up to this size. */
const MAX_EMBED = 4 * 1024 * 1024;

export interface CreatorSession {
  state: CreatorState;
  /** The theme being edited: a saved theme's id, or a folder theme's file. */
  editing?: { id: string; file?: string };
  /** Apply the theme to Discord while editing. */
  live: boolean;
}

/**
 * The theme creator: colour pickers for parts of Discord, a background picture or video, a
 * preview, and Save. The view is built once per session and inputs update the preview in
 * place, so a colour picker stays open while you drag it.
 */
export class ThemeCreatorView {
  private mock!: HTMLElement;
  private mockBg!: HTMLElement;
  private timer?: ReturnType<typeof setTimeout>;
  private previewUrl?: { src: string; url: string; revoke?: () => void };
  private bgSection!: HTMLElement;
  private saving = false;

  constructor(
    private themes: ThemeManager,
    private session: CreatorSession,
    private close: () => void,
  ) {}

  private get state() {
    return this.session.state;
  }

  render(): HTMLElement {
    const s = this.state;
    const nameInput = h("input", {
      type: "text",
      value: s.name,
      maxlength: 80,
      "aria-label": "Theme name",
      "data-key": "creator-name",
      onInput: (e: Event) => {
        s.name = (e.target as HTMLInputElement).value;
        this.changed();
      },
    });
    const authorInput = h("input", {
      type: "text",
      value: s.author,
      maxlength: 80,
      placeholder: "Your name (optional)",
      "aria-label": "Author",
      onInput: (e: Event) => {
        s.author = (e.target as HTMLInputElement).value;
        this.changed();
      },
    });

    const colorRows = new Map<PartId, { picker: HTMLInputElement; text: HTMLInputElement }>();
    const setColor = (id: PartId, hex: string) => {
      s.colors[id] = hex.toLowerCase();
      const row = colorRows.get(id);
      if (row) {
        row.picker.value = hex;
        row.text.value = hex;
      }
    };
    const groups = (["Backgrounds", "Text", "Accents"] as const).map((group) =>
      h(
        "div",
        { class: "creator-group" },
        h("h3", null, group),
        PARTS.filter((p) => p.group === group).map((p) => {
          const picker = h("input", {
            type: "color",
            value: s.colors[p.id],
            "aria-label": p.label,
            "data-part": p.id,
            onInput: (e: Event) => {
              const v = (e.target as HTMLInputElement).value;
              s.colors[p.id] = v;
              text.value = v;
              this.changed();
            },
          });
          const text = h("input", {
            type: "text",
            class: "hex",
            value: s.colors[p.id],
            maxlength: 7,
            spellcheck: false,
            "aria-label": `${p.label} hex code`,
            onChange: (e: Event) => {
              let v = (e.target as HTMLInputElement).value.trim();
              if (!v.startsWith("#")) v = "#" + v;
              if (/^#[0-9a-f]{3}$/i.test(v)) v = "#" + [...v.slice(1)].map((c) => c + c).join("");
              if (HEX.test(v)) {
                setColor(p.id, v);
                this.changed();
              } else text.value = s.colors[p.id];
            },
          });
          colorRows.set(p.id, { picker, text });
          return h("div", { class: "color-row" }, h("div", { class: "color-text" }, h("div", { class: "color-label" }, p.label), p.hint && h("div", { class: "hint" }, p.hint)), picker, text);
        }),
      ),
    );

    const presets = h(
      "div",
      { class: "chips" },
      PRESETS.map((pr) =>
        h(
          "button",
          {
            class: "chip preset",
            type: "button",
            title: `Start from ${pr.name}`,
            onClick: () => {
              for (const p of PARTS) setColor(p.id, pr.colors[p.id]);
              this.changed();
            },
          },
          h("span", { class: "swatches" }, ["serverList", "chat", "text", "accent"].map((k) => h("span", { style: `background:${pr.colors[k as PartId]}` }))),
          pr.name,
        ),
      ),
    );

    const font = h(
      "select",
      {
        "aria-label": "Font",
        onChange: (e: Event) => {
          s.font = (e.target as HTMLSelectElement).value;
          this.changed();
        },
      },
      FONTS.map((f) => h("option", { value: f.value, selected: f.value === s.font }, f.label)),
      !FONTS.some((f) => f.value === s.font) && h("option", { value: s.font, selected: true }, s.font),
    );

    this.bgSection = h("div", { class: "creator-group" });
    this.renderBackground();

    const liveToggle = h("label", { class: "check" }, h("input", {
      type: "checkbox",
      checked: this.session.live,
      onChange: (e: Event) => {
        this.session.live = (e.target as HTMLInputElement).checked;
        this.changed(true);
      },
    }), "Preview on Discord while editing");

    const where = this.themes.native ? "It is saved as a .css file in your themes folder." : "It is saved with your other themes.";
    const controls = h(
      "div",
      { class: "creator-controls" },
      h("div", { class: "field" }, h("label", null, "Name"), nameInput),
      h("div", { class: "field" }, h("label", null, "Made by"), authorInput),
      h("h3", null, "Start from"),
      presets,
      groups,
      this.bgSection,
      h("div", { class: "creator-group" }, h("h3", null, "Text"), h("div", { class: "field" }, h("label", null, "Font"), font)),
    );

    this.mock = this.buildMock();
    const side = h(
      "div",
      { class: "creator-side" },
      h("div", { class: "creator-preview-label" }, "Preview"),
      this.mock,
      liveToggle,
      h("div", { class: "hint" }, where),
      h(
        "div",
        { class: "creator-actions" },
        h("button", { class: "btn", "data-action": "save-theme", onClick: () => this.save() }, this.session.editing ? "Save changes" : "Save theme"),
        h("button", { class: "btn secondary", onClick: () => this.cancel() }, "Cancel"),
        h(
          "button",
          {
            class: "btn secondary",
            title: "Copy the theme's CSS",
            onClick: () =>
              navigator.clipboard?.writeText(buildCss(this.state)).then(
                () => showToast("Copied the theme's CSS", { kind: "success" }),
                () => showToast("Couldn't copy", { kind: "error" }),
              ),
          },
          "Copy CSS",
        ),
      ),
    );

    const view = h(
      "section",
      { class: "creator" },
      h("div", { class: "creator-top" }, h("button", { class: "btn small secondary", onClick: () => this.cancel() }, "← Themes"), h("h2", { class: "creator-title" }, this.session.editing ? "Edit theme" : "Create a theme")),
      h("p", { class: "lede" }, "Pick colours for each part of Discord, add a background picture or video if you like, and save. No code needed."),
      h("div", { class: "creator-layout" }, controls, side),
    );
    this.changed(true);
    return view;
  }

  /* ------------------------------ background ------------------------------ */

  private renderBackground() {
    const s = this.state;
    const bg = s.background;
    const el = this.bgSection;
    while (el.firstChild) el.firstChild.remove();

    const setBg = (next: Background | undefined) => {
      s.background = next;
      this.renderBackground();
      this.changed(true);
    };
    const fileInput = h("input", {
      type: "file",
      accept: "image/*,video/mp4,video/webm,video/quicktime,.mp4,.webm,.mov,.m4v",
      style: "display:none",
      onChange: async (e: Event) => {
        const input = e.target as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";
        if (file) await this.useFile(file);
      },
    });
    const urlInput = h("input", { type: "url", placeholder: "…or paste a link to a picture or video (https://…)", "aria-label": "Background link" });
    const useUrl = () => {
      const src = cleanSrc(urlInput.value);
      if (!src || src.startsWith("data:") || src.startsWith("media/")) return showToast("That isn't an http(s) link", { kind: "error" });
      this.dropPreviewUrl();
      setBg({ ...(bg ?? defaults()), src, kind: guessKind(src) });
    };
    urlInput.addEventListener("keydown", (e) => e.key === "Enter" && useUrl());

    const slider = (label: string, key: "blur" | "dim" | "panels", min: number, max: number, unit: string, hint: string) => {
      const out = h("span", { class: "slider-value" }, `${bg![key]}${unit}`);
      return h(
        "div",
        { class: "field" },
        h("label", null, label, " ", out),
        h("div", { class: "hint" }, hint),
        h("input", {
          type: "range",
          min,
          max,
          value: bg![key],
          "aria-label": label,
          onInput: (e: Event) => {
            bg![key] = Number((e.target as HTMLInputElement).value);
            out.textContent = `${bg![key]}${unit}`;
            this.changed();
          },
        }),
      );
    };

    el.append(h(
      "div",
      null,
      h("h3", null, "Background"),
      h("div", { class: "hint" }, "A picture or video behind Discord. Pictures can be PNG, JPG, GIF or WebP; videos MP4 or WebM. Videos play muted on a loop."),
      h(
        "div",
        { class: "toolbar" },
        fileInput,
        h("button", { class: "btn secondary", onClick: () => fileInput.click() }, bg ? "Choose another file" : "Choose a picture or video"),
        bg && h("button", { class: "btn secondary danger-text", onClick: () => (this.dropPreviewUrl(), setBg(undefined)) }, "Remove background"),
      ),
      h("div", { class: "toolbar" }, urlInput, h("button", { class: "btn secondary", onClick: useUrl }, "Use link")),
      bg &&
        h(
          "div",
          { class: "bg-settings" },
          h("div", { class: "hint bg-current" }, `${bg.kind === "video" ? "Video" : "Picture"}: ${bg.src.startsWith("data:") ? "stored in the theme" : bg.src}`),
          h(
            "div",
            { class: "field" },
            h("label", null, "Fit"),
            h(
              "select",
              {
                onChange: (e: Event) => {
                  bg.fit = (e.target as HTMLSelectElement).value as Background["fit"];
                  this.changed();
                },
              },
              [
                ["cover", "Fill the window"],
                ["contain", "Show all of it"],
                ...(bg.kind === "image" ? [["tile", "Tile"]] : []),
              ].map(([v, l]) => h("option", { value: v, selected: bg.fit === v }, l)),
            ),
          ),
          slider("Blur", "blur", 0, 40, "px", "Softens the background so text stays readable."),
          slider("Darken", "dim", 0, 90, "%", ""),
          slider("Panel opacity", "panels", 0, 100, "%", "How solid the server list, channel list and chat are over the background. Lower shows more of it."),
        ),
    ));
  }

  private async useFile(file: File) {
    const kind = file.type.startsWith("video/") || /\.(mp4|webm|mov|m4v)$/i.test(file.name) ? "video" : "image";
    const native = this.themes.native;
    let src: string;
    try {
      if (native?.writeMedia) {
        showToast(`Saving ${file.name}…`);
        src = await native.writeMedia(file.name.replace(/[^\w.()+-]/g, "_"), file);
      } else {
        if (file.size > MAX_EMBED) {
          showToast(`Without the AesDc client's themes folder, files over ${MAX_EMBED / 1024 / 1024} MB can't be stored. Use a link instead.`, { kind: "error", title: "File too large" });
          return;
        }
        src = await new Promise<string>((resolve, reject) => {
          const r = new FileReader();
          r.onload = () => resolve(String(r.result));
          r.onerror = () => reject(r.error);
          r.readAsDataURL(file);
        });
      }
    } catch (err) {
      showToast(String((err as Error)?.message ?? err), { kind: "error", title: "Couldn't use that file" });
      return;
    }
    this.dropPreviewUrl();
    const url = URL.createObjectURL(file);
    this.previewUrl = { src, url, revoke: () => URL.revokeObjectURL(url) };
    this.state.background = { ...(this.state.background ?? defaults()), src, kind, ...(kind === "video" && this.state.background?.fit === "tile" ? { fit: "cover" as const } : {}) };
    this.renderBackground();
    this.changed(true);
  }

  private dropPreviewUrl() {
    this.previewUrl?.revoke?.();
    this.previewUrl = undefined;
  }

  /* -------------------------------- preview ------------------------------- */

  private buildMock(): HTMLElement {
    const msg = (name: string, color: string, text: (string | Node)[], extra = "") =>
      h("div", { class: `m-msg ${extra}` }, h("span", { class: "m-avatar", style: `background:${color}` }), h("div", null, h("div", null, h("span", { class: "m-name" }, name), h("span", { class: "m-time" }, "Today at 4:20 PM")), h("div", { class: "m-text" }, ...text)));
    return h(
      "div",
      { class: "mock", "aria-label": "Theme preview", role: "img" },
      (this.mockBg = h("div", { class: "m-bg" })),
      h("div", { class: "m-servers" }, h("span", { class: "m-icon accent" }), h("span", { class: "m-sep" }), h("span", { class: "m-icon" }), h("span", { class: "m-icon" }), h("span", { class: "m-icon" })),
      h(
        "div",
        { class: "m-channels" },
        h("div", { class: "m-guild" }, "My Server"),
        h("div", { class: "m-cat" }, "TEXT CHANNELS"),
        h("div", { class: "m-ch selected" }, "# general"),
        h("div", { class: "m-ch hover" }, "# memes"),
        h("div", { class: "m-ch" }, "# art"),
        h("div", { class: "m-ch" }, "🔊 Lounge"),
        h("div", { class: "m-user" }, h("span", { class: "m-avatar small" }), h("div", null, h("div", { class: "m-name" }, "you"), h("div", { class: "m-time" }, "Online"))),
      ),
      h(
        "div",
        { class: "m-chat" },
        h("div", { class: "m-head" }, "# general"),
        h(
          "div",
          { class: "m-msgs" },
          msg("Alex", "#e67e22", ["Has anyone tried the new update?"]),
          msg("Sam", "#2ecc71", ["Yes! Notes are here: ", h("span", { class: "m-link" }, "example.com/notes")], "hover"),
          msg("Jo", "#9b59b6", [h("span", { class: "m-mention" }, "@you"), " are you coming tonight?"], "mentioned"),
        ),
        h("div", { class: "m-box" }, "Message #general"),
        h("div", { class: "m-menu" }, h("div", null, "Mark as read"), h("div", { class: "hover" }, "Copy link"), h("span", { class: "m-btn" }, "Join")),
        h("span", { class: "m-scroll" }),
      ),
    );
  }

  private updateMock() {
    const s = this.state;
    const bg = s.background;
    const alpha = bg ? bg.panels / 100 : 1;
    const style = this.mock.style;
    for (const p of PARTS) style.setProperty(`--m-${p.id}`, p.panel ? rgba(s.colors[p.id], alpha) : s.colors[p.id]);
    style.setProperty("--m-mention-bg", rgba(s.colors.mention, 0.12));
    style.setProperty("--m-font", s.font || "inherit");

    const m = this.mockBg;
    const src = bg?.src;
    const current = m.dataset.src;
    if (src !== current) {
      while (m.firstChild) m.firstChild.remove();
      m.style.backgroundImage = "";
      m.dataset.src = src ?? "";
      if (src) {
        const show = (url: string) => {
          if (m.dataset.src !== src) return;
          if (bg!.kind === "video") {
            const v = h("video", { src: url, autoplay: true, loop: true, muted: true, playsInline: true });
            v.setAttribute("muted", "");
            m.appendChild(v);
            void v.play?.()?.catch?.(() => {});
          } else m.style.backgroundImage = `url("${url.replace(/["\\]/g, (c) => "\\" + c)}")`;
        };
        if (this.previewUrl?.src === src) show(this.previewUrl.url);
        else
          this.themes.resolveMedia(src).then(
            (r) => {
              this.dropPreviewUrl();
              this.previewUrl = { src, ...r };
              show(r.url);
            },
            () => (m.dataset.src = ""),
          );
      }
    }
    if (bg) {
      m.style.filter = `blur(${Math.min(bg.blur, 12) / 2}px) brightness(${1 - bg.dim / 100})`;
      m.style.backgroundSize = bg.fit === "tile" ? "auto" : bg.fit;
      m.style.backgroundRepeat = bg.fit === "tile" ? "repeat" : "no-repeat";
      const v = m.querySelector("video");
      if (v) v.style.objectFit = bg.fit === "contain" ? "contain" : "cover";
    }
  }

  /** Something changed: update the preview now, and Discord shortly after. */
  private changed(now = false) {
    if (this.mock) this.updateMock();
    clearTimeout(this.timer);
    const push = () => this.themes.setPreview(this.session.live ? buildCss(this.state) : null, this.session.editing?.id);
    if (now) push();
    else this.timer = setTimeout(push, 120);
  }

  /* --------------------------------- save --------------------------------- */

  private async save() {
    if (this.saving) return;
    const s = this.state;
    s.name = s.name.trim();
    if (!s.name) return showToast("Give your theme a name first", { kind: "error" });
    const css = buildCss(s);
    const editing = this.session.editing;
    this.saving = true;
    try {
      if (editing && !editing.file) {
        this.themes.updateSaved(editing.id, css);
        showToast(`Saved ${s.name}`, { kind: "success" });
      } else if (this.themes.native) {
        let file = editing?.file;
        if (!file) {
          const taken = new Set(this.themes.listFolder().map((t) => t.file!.toLowerCase()));
          const base = fileName(s.name);
          file = base;
          for (let n = 2; taken.has(file.toLowerCase()); n++) file = base.replace(/\.theme\.css$/, ` (${n}).theme.css`);
        }
        await this.themes.writeFile(file, css);
        showToast(`Saved ${s.name} to your themes folder as ${file}`, { kind: "success" });
      } else {
        this.themes.add(css);
        showToast(`Saved ${s.name}`, { kind: "success" });
      }
    } catch (err) {
      showToast(String((err as Error)?.message ?? err), { kind: "error", title: "Couldn't save the theme" });
      return;
    } finally {
      this.saving = false;
    }
    this.finish();
  }

  private cancel() {
    this.finish();
  }

  private finish() {
    clearTimeout(this.timer);
    this.dropPreviewUrl();
    this.themes.setPreview(null);
    this.close();
  }

  /** Called when the settings view goes away without Save or Cancel. */
  dispose() {
    clearTimeout(this.timer);
    this.dropPreviewUrl();
    this.themes.setPreview(null);
  }
}

function defaults(): Background {
  return { src: "", kind: "image", fit: "cover", blur: 0, dim: 30, panels: 70 };
}
