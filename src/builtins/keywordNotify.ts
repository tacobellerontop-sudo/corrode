import { common } from "../api/common";
import { definePlugin } from "../plugins/types";

/** Example: Flux subscriptions + settings + toasts. */
export default definePlugin({
  id: "keywordNotify",
  name: "Keyword Notify",
  description: "Shows a toast when a message contains one of your keywords, in any channel you can see.",
  authors: ["Corrode"],
  settings: {
    keywords: {
      type: "string",
      label: "Keywords",
      description: "Comma separated. Matches whole words.",
      placeholder: "release, outage, my-name",
      default: "",
    },
    caseSensitive: { type: "boolean", label: "Case sensitive", default: false },
    ignoreOwn: { type: "boolean", label: "Ignore my own messages", default: true },
    duration: { type: "number", label: "Toast duration (seconds)", default: 6, min: 1, max: 60 },
  },
  flux: {
    MESSAGE_CREATE(action, ctx) {
      const msg = action.message;
      if (!msg?.content || action.optimistic) return;
      if (ctx.settings.ignoreOwn && msg.author?.id === common.getCurrentUser()?.id) return;

      const keywords = String(ctx.settings.keywords)
        .split(",")
        .map((k) => k.trim())
        .filter(Boolean);
      if (!keywords.length) return;

      const flags = ctx.settings.caseSensitive ? "u" : "iu";
      const escaped = keywords.map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
      const re = new RegExp(`(?<![\\p{L}\\p{N}_])(${escaped.join("|")})(?![\\p{L}\\p{N}_])`, flags);
      const hit = re.exec(msg.content);
      if (!hit) return;

      const channel = common.ChannelStore?.getChannel?.(msg.channel_id ?? action.channelId);
      const where = channel?.name ? `#${channel.name}` : "a DM";
      const who = msg.author?.global_name ?? msg.author?.username ?? "Someone";
      const preview = msg.content.length > 160 ? `${msg.content.slice(0, 157)}…` : msg.content;
      ctx.toast(preview, { title: `"${hit[1]}" from ${who} in ${where}`, duration: Number(ctx.settings.duration) * 1000 });
    },
  },
});
