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
.notice { padding: 10px 12px; border-radius: 6px; background: rgba(240,178,50,.1); border: 1px solid rgba(240,178,50,.35); margin-bottom: 14px; }
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
