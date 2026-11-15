import { appearancePlugins } from "./appearance";
import { eventPlugins } from "./events";
import hideClutter from "./hideClutter";
import hideTyping from "./hideTyping";
import keywordNotify from "./keywordNotify";
import { pagePlugins } from "./page";
import { sendingPlugins } from "./sending";
import silentTyping from "./silentTyping";
import { utilityPlugins } from "./utility";
import type { PluginDefinition } from "../plugins/types";

/**
 * Every built-in plugin. All are off until the user turns them on.
 *
 * - appearance.ts: stylesheets (hiding things, layout, colours, text)
 * - sending.ts:    changes to what you send
 * - events.ts:     notices and filters driven by Discord's event stream
 * - page.ts:       additions to the page (timestamps, message rows, links, media, message box)
 * - utility.ts:    shortcuts, sounds, reminders and privacy
 */
const group = (category: string, plugins: PluginDefinition<any>[]) => plugins.map((p) => ({ category, ...p }));

export const builtinPlugins: PluginDefinition<any>[] = [
  ...group("Notices and filters", [keywordNotify, hideTyping]),
  ...group("Sending", [silentTyping]),
  ...group("Appearance", [hideClutter, ...appearancePlugins]),
  ...group("Sending", sendingPlugins),
  ...group("Notices and filters", eventPlugins),
  ...group("Chat and media", pagePlugins),
  ...group("Tools and privacy", utilityPlugins),
];
