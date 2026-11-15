import { definePlugin } from "../plugins/types";

/** Example: Flux interception (blocking actions before Discord's stores see them). */
export default definePlugin({
  id: "hideTypingIndicators",
  name: "Hide Typing Indicators",
  description: "Hides \"… is typing\" for other people.",
  authors: ["Corrode"],
  settings: {
    keepInDms: { type: "boolean", label: "Keep typing indicators in DMs", default: true },
  },
  start(ctx) {
    ctx.flux.intercept((action) => {
      if (action.type !== "TYPING_START") return;
      if (ctx.settings.keepInDms && !action.guildId) return;
      return true; // block
    });
  },
});
