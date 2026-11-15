import { filters, waitFor } from "../api/webpack";
import { definePlugin } from "../plugins/types";

/** Example: webpack module finding + function patching. */
export default definePlugin({
  id: "silentTyping",
  name: "Silent Typing",
  description: "Stops Discord from telling others that you're typing.",
  authors: ["Corrode"],
  settings: {
    allowInDms: {
      type: "boolean",
      label: "Still show typing in DMs",
      default: false,
    },
  },
  async start(ctx) {
    const TypingActions = await waitFor(filters.byProps("startTyping", "stopTyping"), { signal: ctx.signal, timeout: 60_000 });
    ctx.patcher.instead(TypingActions, "startTyping", (args, original) => {
      const [channelId] = args;
      if (ctx.settings.allowInDms && isDm(channelId)) return original(...args);
      // Swallow the call: no typing indicator is sent.
    });
  },
});

function isDm(channelId: string): boolean {
  const ChannelStore = (globalThis as any).corrode?.common?.ChannelStore;
  const type = ChannelStore?.getChannel?.(channelId)?.type;
  // 1 = DM, 3 = group DM
  return type === 1 || type === 3;
}
