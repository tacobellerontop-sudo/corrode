import type { PluginDefinition } from "../../plugins/types";
import folderSidebar from "./folderSidebar";
import hiddenChannels from "./hiddenChannels";
import messageLogger from "./messageLogger";
import permissionViewer from "./permissionViewer";

/** The bigger built-in plugins, shown first on the Plugins page under "Power Plugins". */
export const specialPlugins: PluginDefinition<any>[] = [messageLogger, folderSidebar, hiddenChannels, permissionViewer];
