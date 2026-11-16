// Stands in for Discord's own preload: Corrode's preload must still run it.
const { contextBridge } = require("electron");
contextBridge.exposeInMainWorld("DiscordNative", { isFake: true });
