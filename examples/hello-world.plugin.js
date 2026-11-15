/**
 * Example external plugin. Install it from Corrode settings > Plugins by pasting the raw URL
 * of this file, or run in the console:
 *   corrode.plugins.installFromSource(`<file contents>`)
 *
 * External plugins are CommonJS-style scripts: assign your definition to `module.exports`.
 * `corrode` (the public API) is in scope.
 */
module.exports = corrode.definePlugin({
  id: "helloWorld",
  name: "Hello World",
  description: "Greets you when you switch channels, and adds a slash of colour.",
  authors: ["you"],
  version: "1.0.0",

  settings: {
    greeting: { type: "string", label: "Greeting", default: "Welcome to" },
    accent: {
      type: "select",
      label: "Accent colour",
      default: "rust",
      options: [
        { label: "Rust", value: "rust" },
        { label: "Teal", value: "teal" },
      ],
    },
  },

  // Re-rendered whenever a setting changes; removed when the plugin stops.
  css: (s) => `::selection { background: ${s.accent === "teal" ? "#1a9e8f" : "#b7410e"}; color: #fff; }`,

  flux: {
    CHANNEL_SELECT(action, ctx) {
      const channel = corrode.common.ChannelStore?.getChannel(action.channelId);
      if (channel?.name) ctx.toast(`${ctx.settings.greeting} #${channel.name}`);
    },
  },

  start(ctx) {
    ctx.logger.info("Hello from an external plugin!");
  },
});
