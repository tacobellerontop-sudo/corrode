import { appearancePlugins } from "./appearance";
import { eventPlugins } from "./events";
import hideClutter from "./hideClutter";
import hideTyping from "./hideTyping";
import keywordNotify from "./keywordNotify";
import { pagePlugins } from "./page";
import { sendingPlugins } from "./sending";
import silentTyping from "./silentTyping";
import { utilityPlugins } from "./utility";
import { specialPlugins } from "./special";
import type { PluginDefinition } from "../plugins/types";
import { PLUGIN_CATEGORY } from "./categories";

/**
 * Every built-in plugin. All are off until the user turns them on.
 *
 * The files group plugins by how they work (appearance.ts: stylesheets, sending.ts: changes to
 * what you send, events.ts: Discord's event stream, page.ts: additions to the page, utility.ts:
 * shortcuts and tools, special/: bigger plugins that hook deeper into Discord). The Plugins page groups them by what they do instead: see categories.ts.
 */
const all: PluginDefinition<any>[] = [
  ...specialPlugins,
  keywordNotify,
  hideTyping,
  silentTyping,
  hideClutter,
  ...appearancePlugins,
  ...sendingPlugins,
  ...eventPlugins,
  ...pagePlugins,
  ...utilityPlugins,
];

export const builtinPlugins: PluginDefinition<any>[] = all.map((p) => ({ ...p, category: PLUGIN_CATEGORY[p.id] ?? "Utilities" }));
