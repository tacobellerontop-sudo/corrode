import { definePlugin } from "../plugins/types";

/** Example: settings-driven CSS injection. */
const RULES = {
  giftButton: 'button[aria-label="Send a gift"], div:has(> button[aria-label="Send a gift"])',
  stickerButton: 'button[aria-label="Open sticker picker"]',
  appLauncher: '[class*="channelAppLauncher"]',
  nitroTabs: 'li:has(> div > a[href="/store"]), li:has(> div > a[href="/shop"]), a[href="/store"], a[href="/shop"]',
  activeNow: 'aside[class*="nowPlayingColumn"], [class*="nowPlayingColumn"]',
} as const;

export default definePlugin({
  id: "hideClutter",
  name: "Hide Clutter",
  description: "Hides buttons and upsells you don't use. Some selectors match Discord's English labels.",
  authors: ["Corrode"],
  enabledByDefault: false,
  settings: {
    giftButton: { type: "boolean", label: "Hide the gift button", default: true },
    stickerButton: { type: "boolean", label: "Hide the sticker button", default: false },
    appLauncher: { type: "boolean", label: "Hide the app launcher button", default: true },
    nitroTabs: { type: "boolean", label: "Hide Nitro and Shop in the DM list", default: true },
    activeNow: { type: "boolean", label: "Hide the Active Now column", default: false },
  },
  css: (s) =>
    (Object.keys(RULES) as (keyof typeof RULES)[])
      .filter((k) => s[k])
      .map((k) => `${RULES[k]} { display: none !important; }`)
      .join("\n"),
});
