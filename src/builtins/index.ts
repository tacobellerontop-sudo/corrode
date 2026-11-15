import hideClutter from "./hideClutter";
import hideTyping from "./hideTyping";
import keywordNotify from "./keywordNotify";
import silentTyping from "./silentTyping";
import type { PluginDefinition } from "../plugins/types";

export const builtinPlugins: PluginDefinition<any>[] = [silentTyping, keywordNotify, hideTyping, hideClutter];
