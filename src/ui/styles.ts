/**
 * Settings UI styles. Colours fall back to Discord's dark theme but follow Discord's own
 * CSS variables (and therefore the user's theme) when they are available.
 */
export const SETTINGS_CSS = `
:host { all: initial; }
* { box-sizing: border-box; }
.backdrop {
  --bg: var(--background-primary, #313338);
  --bg2: var(--background-secondary, #2b2d31);
  --bg3: var(--background-tertiary, #1e1f22);
  --card: var(--background-secondary-alt, #232428);
  --text: var(--text-normal, #dbdee1);
  --muted: var(--text-muted, #949ba4);
  --header: var(--header-primary, #f2f3f5);
  --accent: #d2692f;
  --accent-hover: #b7561f;
  --danger: #da373c;
  --border: var(--background-modifier-accent, rgba(78,80,88,.48));
  --input: var(--input-background, #1e1f22);
  position: fixed; inset: 0; z-index: 2147483000;
  display: flex; align-items: center; justify-content: center;
  background: rgba(0,0,0,.7);
  font-family: var(--font-primary, "gg sans", "Noto Sans", system-ui, sans-serif);
  font-size: 14px; line-height: 1.4; color: var(--text);
  animation: fade .15s ease-out;
}
@keyframes fade { from { opacity: 0; } }
.modal {
  display: grid; grid-template-columns: 200px 1fr;
  width: min(960px, calc(100vw - 32px)); height: min(680px, calc(100vh - 32px));
  background: var(--bg); border-radius: 10px; overflow: hidden;
  box-shadow: 0 0 0 1px rgba(0,0,0,.2), 0 16px 48px rgba(0,0,0,.4);
}
nav { background: var(--bg2); padding: 20px 10px; display: flex; flex-direction: column; gap: 2px; }
.brand { display: flex; align-items: center; gap: 8px; padding: 0 10px 14px; font-weight: 800; font-size: 16px; color: var(--header); letter-spacing: .02em; }
.brand-mark { width: 22px; height: 22px; border-radius: 6px; background: linear-gradient(135deg, #e0884b, #8a3a12); flex: none; }
.brand small { font-weight: 500; font-size: 11px; color: var(--muted); }
.tab { all: unset; cursor: pointer; padding: 7px 10px; border-radius: 4px; color: var(--muted); font-weight: 500; }
.tab:hover { background: rgba(78,80,88,.3); color: var(--text); }
.tab[aria-selected="true"] { background: rgba(78,80,88,.6); color: var(--header); }
.tab:focus-visible { outline: 2px solid var(--accent); }
.nav-spacer { flex: 1; }
main { position: relative; overflow-y: auto; padding: 24px 28px 32px; }
.close { all: unset; position: absolute; top: 18px; right: 20px; width: 32px; height: 32px; border-radius: 50%; display: grid; place-items: center; cursor: pointer; color: var(--muted); border: 2px solid currentColor; font-size: 16px; font-weight: 700; }
.close:hover { color: var(--header); }
h2 { margin: 0 0 4px; font-size: 20px; color: var(--header); }
.lede { margin: 0 0 18px; color: var(--muted); }
h3 { margin: 22px 0 8px; font-size: 12px; text-transform: uppercase; letter-spacing: .04em; color: var(--muted); }
.toolbar { display: flex; gap: 8px; margin-bottom: 14px; flex-wrap: wrap; }
input[type=text], input[type=number], input[type=url], select, textarea {
  font: inherit; color: var(--text); background: var(--input); border: 1px solid var(--bg3);
  border-radius: 4px; padding: 8px 10px; min-width: 0;
}
input:focus, select:focus, textarea:focus { outline: 2px solid var(--accent); outline-offset: -1px; }
.toolbar input { flex: 1; }
textarea { width: 100%; resize: vertical; font-family: var(--font-code, ui-monospace, Consolas, monospace); font-size: 13px; }
.btn { all: unset; cursor: pointer; padding: 7px 14px; border-radius: 4px; background: var(--accent); color: #fff; font-weight: 500; text-align: center; white-space: nowrap; }
.btn:hover { background: var(--accent-hover); }
.btn:focus-visible { outline: 2px solid var(--header); }
.btn.secondary { background: #4e5058; } .btn.secondary:hover { background: #6d6f78; }
.btn.danger { background: var(--danger); } .btn.danger:hover { background: #a12828; }
.btn.small { padding: 4px 10px; font-size: 13px; }
.btn[disabled] { opacity: .5; pointer-events: none; }
.cards { display: grid; gap: 8px; }
.card { background: var(--bg2); border: 1px solid var(--border); border-radius: 8px; padding: 12px 14px; }
.card-head { display: flex; gap: 12px; align-items: flex-start; }
.card-main { flex: 1; min-width: 0; }
.card-title { font-weight: 600; color: var(--header); display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
.card-desc { color: var(--muted); margin-top: 2px; overflow-wrap: anywhere; }
.badge { font-size: 11px; font-weight: 600; padding: 1px 6px; border-radius: 3px; background: var(--bg3); color: var(--muted); text-transform: uppercase; letter-spacing: .03em; }
.badge.warn { background: rgba(240,178,50,.15); color: #f0b232; }
.badge.err { background: rgba(242,63,67,.15); color: #f23f43; }
.card-actions { display: flex; gap: 6px; margin-top: 8px; flex-wrap: wrap; }
.settings { margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--border); display: grid; gap: 14px; }
.field label { display: block; font-weight: 600; color: var(--header); margin-bottom: 4px; }
.field .hint { color: var(--muted); font-size: 13px; margin-bottom: 6px; }
.field-row { display: flex; align-items: center; gap: 12px; }
.field-row .field-text { flex: 1; }
.field input[type=text], .field input[type=number], .field select { width: 100%; }
.switch { all: unset; flex: none; position: relative; width: 40px; height: 24px; border-radius: 12px; background: #80848e; cursor: pointer; transition: background .15s; }
.switch::after { content: ""; position: absolute; top: 3px; left: 3px; width: 18px; height: 18px; border-radius: 50%; background: #fff; transition: transform .15s; }
.switch[aria-checked="true"] { background: #23a55a; }
.switch[aria-checked="true"]::after { transform: translateX(16px); }
.switch:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.switch[disabled] { opacity: .5; cursor: not-allowed; }
.empty { color: var(--muted); padding: 24px; text-align: center; border: 1px dashed var(--border); border-radius: 8px; }
.kv { display: grid; grid-template-columns: max-content 1fr; gap: 6px 16px; }
.kv dt { color: var(--muted); } .kv dd { margin: 0; color: var(--text); overflow-wrap: anywhere; }
code { font-family: var(--font-code, ui-monospace, Consolas, monospace); background: var(--bg3); padding: 1px 4px; border-radius: 3px; font-size: 13px; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 16px; }
.chip { all: unset; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 999px; background: var(--bg2); border: 1px solid var(--border); color: var(--text); font-size: 13px; font-weight: 500; }
.chip:hover { border-color: var(--muted); }
.chip[aria-pressed="true"] { background: var(--accent); border-color: var(--accent); color: #fff; }
.chip:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.chip-count { font-size: 11px; opacity: .75; }
.groups { display: grid; gap: 18px; }
.group-head { all: unset; cursor: pointer; display: flex; align-items: baseline; flex-wrap: wrap; gap: 4px 8px; width: 100%; margin-bottom: 8px; }
.group-head:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.group-caret { color: var(--muted); width: 12px; }
.group-name { font-size: 16px; font-weight: 700; color: var(--header); }
.group-count { font-size: 12px; color: var(--muted); }
.group-blurb { flex-basis: 100%; padding-left: 20px; color: var(--muted); font-size: 13px; }
.notice { padding: 10px 12px; border-radius: 6px; background: rgba(240,178,50,.1); border: 1px solid rgba(240,178,50,.35); margin-bottom: 14px; }
/* Themes page */
.create-card { display: flex; align-items: center; gap: 14px; padding: 14px 16px; margin-bottom: 8px; border-radius: 8px; border: 1px solid var(--border);
  background: linear-gradient(120deg, rgba(210,105,47,.18), rgba(88,101,242,.12)), var(--bg2); }
.create-art { display: grid; grid-template-columns: 1fr 1fr; gap: 3px; flex: none; }
.create-art span { width: 14px; height: 14px; border-radius: 4px; }
.hint { color: var(--muted); font-size: 13px; }
p.hint { margin: 0 0 10px; }
code.path { user-select: all; overflow-wrap: anywhere; }
/* Theme creator */
.creator-top { display: flex; align-items: center; gap: 12px; margin-bottom: 6px; }
.creator-top .creator-title { margin: 0; display: block; }
.creator-layout { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 20px; align-items: start; }
.creator-side { position: sticky; top: 0; display: grid; gap: 10px; }
.creator-preview-label { font-size: 12px; text-transform: uppercase; letter-spacing: .04em; color: var(--muted); font-weight: 700; }
.creator-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.creator-controls { display: grid; gap: 12px; min-width: 0; }
.creator-controls h3 { margin: 10px 0 4px; }
.creator-group { display: grid; gap: 6px; }
.color-row { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 6px; background: var(--bg2); }
.color-text { flex: 1; min-width: 0; }
.color-label { color: var(--header); font-weight: 600; }
input[type=color] { flex: none; width: 40px; height: 30px; padding: 0; border: 1px solid var(--border); border-radius: 6px; background: none; cursor: pointer; }
input[type=color]::-webkit-color-swatch-wrapper { padding: 2px; }
input[type=color]::-webkit-color-swatch { border: none; border-radius: 4px; }
input.hex { width: 84px; flex: none; font-family: var(--font-code, ui-monospace, Consolas, monospace); font-size: 13px; padding: 5px 8px; }
input[type=range] { width: 100%; accent-color: var(--accent); }
.slider-value { color: var(--muted); font-weight: 500; }
.chip.preset { gap: 8px; }
.swatches { display: inline-flex; }
.swatches span { width: 10px; height: 14px; }
.swatches span:first-child { border-radius: 3px 0 0 3px; } .swatches span:last-child { border-radius: 0 3px 3px 0; }
.check { display: flex; align-items: center; gap: 8px; cursor: pointer; }
.check input { accent-color: var(--accent); width: 16px; height: 16px; }
.bg-settings { display: grid; gap: 10px; padding: 10px; border-radius: 8px; background: var(--bg2); }
.bg-current { overflow-wrap: anywhere; }
.danger-text { color: #ff8a8d; }
/* The creator's preview: a small Discord drawn with the picked colours. */
.mock { position: relative; display: grid; grid-template-columns: 34px 92px 1fr; height: 260px; border-radius: 8px; overflow: hidden; font-size: 10px; line-height: 1.3;
  font-family: var(--m-font); border: 1px solid var(--border); isolation: isolate; background: #000; }
.m-bg { position: absolute; inset: 0; z-index: -1; background-position: center; background-size: cover; }
.m-bg video { width: 100%; height: 100%; object-fit: cover; display: block; }
.m-servers { background: var(--m-serverList); display: flex; flex-direction: column; align-items: center; gap: 5px; padding-top: 6px; }
.m-icon { width: 22px; height: 22px; border-radius: 50%; background: var(--m-channelList); }
.m-icon.accent { background: var(--m-accent); border-radius: 7px; }
.m-sep { width: 14px; height: 1px; background: var(--m-borders); }
.m-channels { background: var(--m-channelList); display: flex; flex-direction: column; color: var(--m-channels); min-width: 0; }
.m-guild { color: var(--m-headers); font-weight: 700; padding: 7px 7px; border-bottom: 1px solid var(--m-borders); }
.m-cat { font-size: 8px; font-weight: 700; padding: 8px 7px 2px; color: var(--m-channels); }
.m-ch { margin: 1px 4px; padding: 3px 4px; border-radius: 3px; white-space: nowrap; overflow: hidden; }
.m-ch.selected { background: var(--m-selected); color: var(--m-channelsActive); }
.m-ch.hover { background: var(--m-hover); color: var(--m-channelsActive); }
.m-user { margin-top: auto; display: flex; gap: 4px; align-items: center; padding: 5px; background: var(--m-userPanel); }
.m-chat { position: relative; background: var(--m-chat); display: flex; flex-direction: column; min-width: 0; color: var(--m-text); }
.m-head { color: var(--m-headers); font-weight: 700; padding: 7px 8px; border-bottom: 1px solid var(--m-borders); }
.m-msgs { flex: 1; display: flex; flex-direction: column; justify-content: flex-end; gap: 2px; padding: 4px 0; overflow: hidden; }
.m-msg { display: flex; gap: 6px; padding: 3px 8px; }
.m-msg.hover { background: var(--m-hover); }
.m-msg.mentioned { background: var(--m-mention-bg); box-shadow: inset 2px 0 0 var(--m-mention); }
.m-avatar { width: 18px; height: 18px; border-radius: 50%; flex: none; background: var(--m-accent); }
.m-avatar.small { width: 14px; height: 14px; }
.m-name { color: var(--m-headers); font-weight: 600; margin-right: 4px; }
.m-time { color: var(--m-muted); font-size: 8px; }
.m-link { color: var(--m-links); }
.m-mention { color: var(--m-accent); background: color-mix(in srgb, var(--m-accent) 25%, transparent); border-radius: 3px; padding: 0 2px; }
.m-box { margin: 0 8px 8px; padding: 6px 8px; border-radius: 6px; background: var(--m-messageBox); color: var(--m-muted); }
.m-menu { position: absolute; right: 10px; top: 30px; width: 84px; padding: 4px; border-radius: 5px; background: var(--m-menus); color: var(--m-text); box-shadow: 0 4px 12px rgba(0,0,0,.35); display: grid; gap: 1px; }
.m-menu > div { padding: 3px 5px; border-radius: 3px; }
.m-menu > .hover { background: var(--m-accent); color: #fff; }
.m-btn { margin-top: 3px; text-align: center; padding: 3px; border-radius: 3px; background: var(--m-accent); color: #fff; font-weight: 600; }
.m-scroll { position: absolute; right: 2px; top: 40px; width: 4px; height: 50px; border-radius: 2px; background: var(--m-scrollbar); }
@media (max-width: 860px) { .creator-layout { grid-template-columns: 1fr; } .creator-side { position: static; } }
/* Embedded in Discord's own settings page: no window chrome, Discord shows the page title. */
:host { display: block; }
.backdrop.embedded { position: static; display: block; background: none; animation: none; z-index: auto; }
.embedded main { padding: 0; overflow: visible; }
.embedded h2 { display: none; }
@media (max-width: 640px) {
  .modal { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
  nav { flex-direction: row; overflow-x: auto; padding: 10px; }
  .brand, .nav-spacer { display: none; }
  main { padding: 20px 16px; }
}
`;
