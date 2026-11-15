/**
 * Appearance plugins: each one is a stylesheet that is on the page while the plugin is enabled.
 *
 * Discord's class names look like `name_ab12cd`; the suffix changes between releases, so the
 * selectors here match the stable part with `[class*="name"]`. A few buttons can only be found by
 * their English `aria-label`; those plugins say so in their description.
 */
import { definePlugin, type PluginDefinition, type SettingsSchema } from "../plugins/types";
import { clamp, cssValue } from "./lib/helpers";

const hide = (...selectors: string[]) => `${selectors.join(",\n")} { display: none !important; }`;

function style(id: string, name: string, description: string, css: string): PluginDefinition<any> {
  return definePlugin({ id, name, description, authors: ["Corrode"], css });
}

function styled<S extends SettingsSchema>(id: string, name: string, description: string, settings: S, css: (s: any) => string): PluginDefinition<any> {
  return definePlugin({ id, name, description, authors: ["Corrode"], settings, css }) as PluginDefinition<any>;
}

const CHAT = 'ol[data-list-id="chat-messages"]';
const ROW = 'li[class*="messageListItem"]';
const MARKUP = '[class*="markup_"]';
const GUILDS = 'nav[class*="guilds"]';
const SIDEBAR = 'div[class*="sidebar_"]';
const MEMBERS = '[class*="membersWrap"]';
const toolbarButton = (label: string) => `[class*="toolbar"] [aria-label="${label}"]`;
const chatButton = (label: string) => `[class*="channelTextArea"] button[aria-label="${label}"], [class*="channelTextArea"] div:has(> button[aria-label="${label}"])`;
const guildNavItem = (id: string) => `[class*="listItem"]:has([data-list-item-id="guildsnav___${id}"])`;

export const appearancePlugins: PluginDefinition<any>[] = [
  /* ---------------------------- hide: cosmetics ---------------------------- */
  style("hideAvatarDecorations", "Hide Avatar Decorations", "Removes the frames and effects around avatars.", hide('[class*="avatarDecoration"]')),
  style("hideProfileEffects", "Hide Profile Effects", "Removes the animated effects on profiles.", hide('[class*="profileEffects"]')),
  style("hideNameplates", "Hide Nameplates", "Removes the decorative backgrounds behind names in the member and DM lists.", hide('[class*="nameplated"] > [class*="container"]:has(video, img)', '[class*="nameplate_"]')),
  style("hideServerTags", "Hide Server Tags", "Removes the server tags shown next to names.", hide('[class*="chipletContainer"]', '[class*="clanTag"]', '[class*="guildTag"]')),
  style("hideRoleIcons", "Hide Role Icons", "Removes the small role icons next to names.", hide('[class*="roleIcon"]')),
  style("hideBotTags", "Hide App Tags", "Removes the APP/BOT label next to bot names.", hide('[class*="botTag"]')),
  style("hideNewMemberBadges", "Hide New Member Badges", "Removes the leaf icon shown for new server members.", hide('[class*="newMemberBadge"]')),
  style("hideProfileBadges", "Hide Profile Badges", "Removes the badge row from profiles.", hide('[class*="profileBadges"]', '[class*="badgeList"]')),
  style(
    "calmSuperReactions",
    "Calm Super Reactions",
    "Stops super reactions from playing their effects and hides the button that adds them. The button is found by its English label.",
    `${hide('[class*="effectsWrapper"]', '[class*="burstGlow"]', '[aria-label="Add Super Reaction"]')}\n[class*="reaction_"][class*="shakeReaction"] { animation: none !important; }`,
  ),
  style("hideEditedTag", "Hide Edited Tag", 'Removes the "(edited)" marker from messages.', hide('[class*="edited_"]')),
  style("hideReactions", "Hide Reactions", "Removes reactions under messages.", hide('[class*="reactions_"]')),
  style("hideStickers", "Hide Stickers", "Removes stickers from chat.", hide(`${CHAT} [class*="stickerAsset"]`, `${CHAT} [class*="clickableSticker"]`)),
  style("hideEmbeds", "Hide Link Embeds", "Removes the previews Discord builds for links. Uploaded files stay.", hide(`${CHAT} article[class*="embed"]`)),
  style("hideReplyPreviews", "Hide Reply Previews", "Removes the quoted line above replies.", hide(`${CHAT} [class*="repliedMessage"]`)),
  style("hideWaveButton", "Hide Wave Button", 'Removes the "wave to say hi" sticker prompt on join messages and empty DMs.', hide('[class*="welcomeCTA"]')),
  style("hideChatAvatars", "Hide Chat Avatars", "Removes avatars from messages for a denser chat.", `${CHAT} img[class*="avatar_"], ${CHAT} [class*="avatarDecoration"] { display: none !important; }\n${CHAT} [class*="cozy_"][class*="wrapper_"] { padding-left: 16px !important; }`),

  /* ----------------------------- hide: chrome ------------------------------ */
  style("hideDownloadButton", "Hide Download Apps Button", "Removes the download button at the bottom of the server list.", hide(guildNavItem("app-download-button"))),
  style("hideDiscoverButton", "Hide Discover Button", "Removes the compass button at the bottom of the server list.", hide(guildNavItem("guild-discover-button"))),
  style("hideAddServerButton", "Hide Add Server Button", "Removes the plus button at the bottom of the server list.", hide(guildNavItem("create-join-button"))),
  style("hideQuests", "Hide Quests", "Removes Quests from the DM list and the quest promotion above your user panel.", hide('a[href="/quest-home"]', 'li:has(> div > a[href="/quest-home"])', '[class*="questsContainer"]', '[class*="questBar"]', '[class*="questPromo"]')),
  style("hideLibrary", "Hide Library", "Removes the Library tab from the DM list.", hide('a[href="/library"]', 'li:has(> div > a[href="/library"])')),
  style("hideHelpButton", "Hide Help Button", "Removes the question mark at the top right.", hide('[class*="toolbar"] a[href^="https://support.discord.com"]')),
  style("hideInboxButton", "Hide Inbox Button", "Removes the inbox button at the top right. Found by its English label.", hide(toolbarButton("Inbox"))),
  style("hideThreadsButton", "Hide Threads Button", "Removes the threads button from the channel header. Found by its English label.", hide(toolbarButton("Threads"))),
  style("hidePinsButton", "Hide Pins Button", "Removes the pinned messages button from the channel header. Found by its English label.", hide(toolbarButton("Pinned Messages"))),
  style("hideNotificationBell", "Hide Notification Bell", "Removes the notification settings button from the channel header. Found by its English label.", hide(toolbarButton("Notification Settings"))),
  style("hideGifButton", "Hide GIF Button", "Removes the GIF button from the message box. Found by its English label.", hide(chatButton("Open GIF picker"))),
  style("hideActivityButton", "Hide Activities Button", "Removes the Activities rocket from voice controls. Found by its English label.", hide('button[aria-label="Start an Activity"]', 'button[aria-label="Start Activity"]')),
  style("hideSoundboardButton", "Hide Soundboard Button", "Removes the soundboard button from voice controls. Found by its English label.", hide('button[aria-label="Open Soundboard"]', 'button[aria-label="Soundboard"]')),
  style("hideTopBanners", "Hide Top Banners", "Removes the coloured announcement bars Discord shows across the top of the window.", hide('div[class^="notice_"]', 'div[class*=" notice_"]')),
  style("hideChannelNotices", "Hide Channel List Notices", "Removes promotional cards at the bottom of the channel list.", hide('[class*="channelNotice"]', `${SIDEBAR} [class*="gradientContainer"]`)),
  style("hideServerBanner", "Hide Server Banners", "Removes the banner image at the top of a server's channel list.", hide(`${SIDEBAR} [class*="bannerImage"]`, `${SIDEBAR} [class*="animatedContainer"]`)),
  style("hideBoostBar", "Hide Boost Progress", "Removes the server boost goal bar from the channel list.", hide(`${SIDEBAR} [class*="progressContainer"]`, `${SIDEBAR} [class*="boostCount"]`)),
  style("hideServerShortcuts", "Hide Server Shortcuts", "Removes Browse Channels, Channels & Roles and Server Guide from the top of the channel list.", hide('li:has(> div a[href$="/channel-browser"])', 'li:has(> div a[href$="/customize-community"])', 'li:has(> div a[href$="/@home"])')),
  style("hideMutedChannels", "Hide Muted Channels", "Removes channels you muted from the channel list.", hide(`${SIDEBAR} li:has([class*="modeMuted"])`)),
  style("hideActivityPanel", "Hide Activity Panel", "Removes the game activity card above your user panel.", hide('[class*="panels"] [class*="activityPanel"]')),
  style("hideUnreadBadges", "Hide Server Badges", "Removes the red mention counters from server icons.", hide(`${GUILDS} [class*="lowerBadge"]`, `${GUILDS} [class*="upperBadge"]`)),
  style("hideUnreadPills", "Hide Unread Pills", "Removes the white unread markers beside server icons.", hide(`${GUILDS} [class*="pill_"]`)),
  style("hideNewMessagesBar", "Hide New Messages Bar", 'Removes the "new messages since" bar at the top of chat.', hide('[class*="newMessagesBar"]')),
  style("hideJumpToPresent", "Hide Jump To Present", "Removes the bar that appears when you scroll up in chat.", hide('[class*="jumpToPresentBar"]')),
  style("hideMentionJumpBars", "Hide Mention Jump Bars", "Removes the floating unread and mentions buttons in the channel and server lists.", hide('[class*="unreadMentionsBar"]', '[class*="unreadMentionsIndicator"]', `${SIDEBAR} [class*="unreadBar"]`)),
  style("hideScrollbars", "Hide Scrollbars", "Hides scrollbars everywhere. Scrolling still works.", "::-webkit-scrollbar { display: none !important; }\n* { scrollbar-width: none !important; }"),
  style("hideChannelIcons", "Hide Channel Icons", "Removes the # and speaker icons from the channel list.", hide(`${SIDEBAR} [class*="containerDefault"] [class*="iconContainer"]`)),

  /* --------------------------------- text ---------------------------------- */
  styled(
    "customFont",
    "Custom Font",
    "Uses fonts installed on your computer for Discord's text and code.",
    {
      font: { type: "string", label: "Font", description: "A font name, for example Inter or Segoe UI.", default: "", placeholder: "Inter" },
      codeFont: { type: "string", label: "Code font", default: "", placeholder: "JetBrains Mono" },
    },
    (s) => {
      const font = cssValue(s.font, "");
      const code = cssValue(s.codeFont, "");
      return `:root, .theme-dark, .theme-light, .visual-refresh {\n${font ? `  --font-primary: "${font.replace(/"/g, "")}", "gg sans", sans-serif !important;\n  --font-display: "${font.replace(/"/g, "")}", "gg sans", sans-serif !important;\n` : ""}${code ? `  --font-code: "${code.replace(/"/g, "")}", Consolas, monospace !important;\n` : ""}}`;
    },
  ),
  styled(
    "chatFontSize",
    "Chat Font Size",
    "Sets the message text size to an exact number of pixels.",
    { size: { type: "number", label: "Size (px)", default: 16, min: 10, max: 32 } },
    (s) => `${CHAT} ${MARKUP} { font-size: ${clamp(s.size, 10, 32, 16)}px !important; line-height: 1.375 !important; }`,
  ),
  styled(
    "lineSpacing",
    "Line Spacing",
    "Changes the space between lines of message text.",
    { height: { type: "number", label: "Line height", description: "1.375 is Discord's default.", default: 1.6, min: 1, max: 3, step: 0.05 } },
    (s) => `${CHAT} ${MARKUP} { line-height: ${clamp(s.height, 1, 3, 1.6)} !important; }`,
  ),
  styled(
    "messageSpacing",
    "Message Spacing",
    "Changes the gap between message groups.",
    { gap: { type: "number", label: "Gap (px)", default: 8, min: 0, max: 48 } },
    (s) => `${CHAT} [class*="groupStart"] { margin-top: ${clamp(s.gap, 0, 48, 8)}px !important; }`,
  ),
  styled(
    "emojiSize",
    "Emoji Size",
    "Sets how large emoji are in messages.",
    {
      inline: { type: "number", label: "Emoji inside text (px)", default: 22, min: 12, max: 64 },
      jumbo: { type: "number", label: "Emoji-only messages (px)", default: 48, min: 16, max: 128 },
    },
    (s) =>
      `${CHAT} ${MARKUP} img.emoji:not([class*="jumboable"]) { width: ${clamp(s.inline, 12, 64, 22)}px !important; height: ${clamp(s.inline, 12, 64, 22)}px !important; }\n${CHAT} ${MARKUP} img.emoji[class*="jumboable"] { width: ${clamp(s.jumbo, 16, 128, 48)}px !important; height: ${clamp(s.jumbo, 16, 128, 48)}px !important; }`,
  ),
  style("underlineLinks", "Underline Links", "Always underlines links in messages so they stand out from text.", `${CHAT} ${MARKUP} a { text-decoration: underline !important; }`),
  style("plainUsernames", "Plain Usernames", "Shows every name in the normal text colour instead of role colours.", `${CHAT} [class*="username_"], ${MEMBERS} [class*="username_"], ${MEMBERS} [class*="name_"] span { color: var(--header-primary, #f2f3f5) !important; background: none !important; -webkit-text-fill-color: currentColor !important; }`),
  style("boldUsernames", "Bold Usernames", "Makes names in chat bold.", `${CHAT} [class*="username_"] { font-weight: 700 !important; }`),
  style("alwaysShowTimestamps", "Always Show Timestamps", "Shows the time on every message, not only when you hover it.", `${CHAT} [class*="timestampVisibleOnHover"] { opacity: 1 !important; }`),
  style("hideTimestamps", "Hide Timestamps", "Removes times from messages.", hide(`${CHAT} [class*="timestamp_"]`)),
  styled(
    "selectionColor",
    "Selection Colour",
    "Changes the colour of selected text.",
    {
      background: { type: "string", label: "Highlight colour", default: "#d2692f", placeholder: "#d2692f" },
      text: { type: "string", label: "Text colour", default: "#ffffff", placeholder: "#ffffff" },
    },
    (s) => `::selection { background: ${cssValue(s.background, "#d2692f")} !important; color: ${cssValue(s.text, "#fff")} !important; }`,
  ),

  /* ------------------------------- code blocks ------------------------------ */
  style("wrapCodeBlocks", "Wrap Code Blocks", "Wraps long lines in code blocks instead of scrolling sideways.", `${CHAT} pre code { white-space: pre-wrap !important; word-break: break-word !important; }`),
  styled(
    "codeBlockHeight",
    "Limit Code Block Height",
    "Makes long code blocks scroll inside a box instead of filling the chat.",
    { height: { type: "number", label: "Maximum height (px)", default: 400, min: 80, max: 2000 } },
    (s) => `${CHAT} pre code { display: block !important; max-height: ${clamp(s.height, 80, 2000, 400)}px !important; overflow: auto !important; }`,
  ),
  styled(
    "codeFontSize",
    "Code Font Size",
    "Sets the text size inside code blocks and inline code.",
    { size: { type: "number", label: "Size (px)", default: 13, min: 9, max: 28 } },
    (s) => `${CHAT} code { font-size: ${clamp(s.size, 9, 28, 13)}px !important; line-height: 1.4 !important; }`,
  ),

  /* --------------------------------- media ---------------------------------- */
  styled(
    "blurMedia",
    "Blur Media",
    "Blurs images and videos in chat until you hover them.",
    { amount: { type: "number", label: "Blur (px)", default: 24, min: 4, max: 80 } },
    (s) =>
      `${CHAT} [class*="imageWrapper"] img, ${CHAT} [class*="imageWrapper"] video, ${CHAT} [class*="embedVideo"] video { filter: blur(${clamp(s.amount, 4, 80, 24)}px) !important; transition: filter .15s !important; }\n${CHAT} [class*="imageWrapper"]:hover img, ${CHAT} [class*="imageWrapper"]:hover video, ${CHAT} [class*="embedVideo"]:hover video { filter: none !important; }`,
  ),
  styled(
    "smallMedia",
    "Smaller Media",
    "Caps how tall images and videos can be in chat.",
    { height: { type: "number", label: "Maximum height (px)", default: 200, min: 60, max: 1000 } },
    (s) => {
      const h = clamp(s.height, 60, 1000, 200);
      return `${CHAT} [class*="imageWrapper"], ${CHAT} [class*="imageWrapper"] img, ${CHAT} [class*="imageWrapper"] video { max-height: ${h}px !important; }\n${CHAT} [class*="imageWrapper"] img, ${CHAT} [class*="imageWrapper"] video { width: auto !important; object-fit: contain !important; }`;
    },
  ),
  style("revealSpoilersOnHover", "Reveal Spoilers On Hover", "Shows spoiler text while the pointer is over it, without clicking.", `${CHAT} span[class*="spoilerContent"][class*="hidden"]:hover { background-color: var(--background-modifier-accent, rgba(78,80,88,.48)) !important; }\n${CHAT} span[class*="spoilerContent"][class*="hidden"]:hover [class*="inlineContent"] { opacity: 1 !important; }`),
  style("roundMedia", "Rounder Media", "Gives images, videos and embeds larger rounded corners.", `${CHAT} [class*="imageWrapper"], ${CHAT} article[class*="embed"], ${CHAT} [class*="imageWrapper"] img, ${CHAT} [class*="imageWrapper"] video { border-radius: 16px !important; }`),

  /* --------------------------------- layout --------------------------------- */
  styled(
    "chatWidth",
    "Readable Chat Width",
    "Caps the width of the message list and centres it, which is easier to read on wide screens.",
    { width: { type: "number", label: "Maximum width (px)", default: 900, min: 480, max: 2400, step: 20 } },
    (s) => `${CHAT} { max-width: ${clamp(s.width, 480, 2400, 900)}px !important; margin-left: auto !important; margin-right: auto !important; }\n[class*="chatContent"] form { max-width: ${clamp(s.width, 480, 2400, 900)}px !important; margin-left: auto !important; margin-right: auto !important; width: 100% !important; }`,
  ),
  styled(
    "sidebarWidth",
    "Channel List Width",
    "Sets the width of the channel and DM list.",
    { width: { type: "number", label: "Width (px)", default: 200, min: 120, max: 480, step: 4 } },
    (s) => `:root { --custom-guild-sidebar-width: ${clamp(s.width, 120, 480, 200)}px !important; }\n${SIDEBAR} { width: calc(${clamp(s.width, 120, 480, 200)}px + 72px) !important; }\n${SIDEBAR} [class*="sidebarList"] { width: ${clamp(s.width, 120, 480, 200)}px !important; }`,
  ),
  styled(
    "memberListWidth",
    "Member List Width",
    "Sets the width of the member list.",
    { width: { type: "number", label: "Width (px)", default: 200, min: 120, max: 480, step: 4 } },
    (s) => `${MEMBERS}, ${MEMBERS} [class*="members_"] { width: ${clamp(s.width, 120, 480, 200)}px !important; min-width: ${clamp(s.width, 120, 480, 200)}px !important; }`,
  ),
  style("autoHideMemberList", "Auto-hide Member List", "Shrinks the member list to a thin strip that opens when you hover it.", `${MEMBERS} { width: 14px !important; min-width: 14px !important; transition: width .15s ease, min-width .15s ease !important; overflow: hidden !important; }\n${MEMBERS}:hover { width: 240px !important; min-width: 240px !important; }`),
  style("autoHideServerList", "Auto-hide Server List", "Shrinks the server list to a thin strip that opens when you hover it.", `${GUILDS} { width: 10px !important; transition: width .15s ease !important; overflow: hidden !important; }\n${GUILDS}:hover { width: 72px !important; }`),
  style("compactChannelList", "Compact Channel List", "Tightens the channel list so more channels fit on screen.", `${SIDEBAR} [class*="containerDefault"] { padding-top: 0 !important; }\n${SIDEBAR} [class*="containerDefault"] a { padding-top: 3px !important; padding-bottom: 3px !important; }\n${SIDEBAR} [class*="containerDefault"] [class*="link_"] { min-height: 26px !important; }`),
  style("compactMemberList", "Compact Member List", "Tightens the member list so more people fit on screen.", `${MEMBERS} [class*="member_"] { height: 34px !important; padding-top: 0 !important; padding-bottom: 0 !important; }\n${MEMBERS} [class*="member_"] [class*="layout"] { height: 32px !important; }\n${MEMBERS} [class*="member_"] [class*="subText"], ${MEMBERS} [class*="member_"] [class*="activity_"] { display: none !important; }`),
  style("squareAvatars", "Square Avatars", "Shows avatars as rounded squares instead of circles.", `${CHAT} img[class*="avatar_"] { border-radius: 22% !important; }\n[class*="avatar_"] foreignObject, [class*="wrapper_"] > svg[class*="mask"] > foreignObject { mask: none !important; -webkit-mask: none !important; border-radius: 22% !important; }\n[class*="avatar_"] foreignObject img { border-radius: 22% !important; }`),
  style("squareServerIcons", "Square Server Icons", "Shows server icons as rounded squares that don't change shape on hover.", `${GUILDS} [class*="listItem"] foreignObject { mask: none !important; -webkit-mask: none !important; border-radius: 26% !important; overflow: hidden !important; }\n${GUILDS} [class*="listItem"] [class*="icon_"], ${GUILDS} [class*="listItem"] [class*="childWrapper"] { border-radius: 26% !important; }`),
  style("sharpCorners", "Sharp Corners", "Removes rounded corners from the whole interface.", "*, *::before, *::after { border-radius: 0 !important; }"),
  styled(
    "uiScale",
    "Interface Scale",
    "Makes the whole interface larger or smaller.",
    { scale: { type: "number", label: "Scale (%)", default: 100, min: 60, max: 180, step: 5 } },
    (s) => `html { zoom: ${clamp(s.scale, 60, 180, 100) / 100} !important; }`,
  ),

  /* --------------------------------- colour --------------------------------- */
  styled(
    "accentColor",
    "Accent Colour",
    "Replaces Discord's blurple on buttons, links and highlights.",
    { color: { type: "string", label: "Colour", default: "#d2692f", placeholder: "#d2692f" } },
    (s) => {
      const c = cssValue(s.color, "#d2692f");
      const names = ["--brand-experiment", "--brand-500", "--brand-560", "--brand-530", "--brand-360", "--brand-260", "--control-brand-foreground", "--text-brand", "--background-brand", "--bg-brand", "--button-filled-brand-background", "--mention-foreground"];
      return `:root, .theme-dark, .theme-light, .visual-refresh {\n${names.map((n) => `  ${n}: ${c} !important;`).join("\n")}\n  --button-filled-brand-background-hover: color-mix(in srgb, ${c} 85%, black) !important;\n  --button-filled-brand-background-active: color-mix(in srgb, ${c} 70%, black) !important;\n  --text-link: color-mix(in srgb, ${c} 70%, white) !important;\n  --mention-background: color-mix(in srgb, ${c} 30%, transparent) !important;\n}`;
    },
  ),
  styled(
    "mentionColor",
    "Mention Highlight Colour",
    "Changes the colour of messages that mention you.",
    { color: { type: "string", label: "Colour", default: "#f0b232", placeholder: "#f0b232" } },
    (s) => {
      const c = cssValue(s.color, "#f0b232");
      return `${CHAT} [class*="mentioned_"] { background-color: color-mix(in srgb, ${c} 12%, transparent) !important; }\n${CHAT} [class*="mentioned_"]::before { background-color: ${c} !important; }\n${CHAT} [class*="mentioned_"]:hover { background-color: color-mix(in srgb, ${c} 18%, transparent) !important; }`;
    },
  ),
  style(
    "amoledBlack",
    "AMOLED Black",
    "Turns the dark theme's backgrounds pure black.",
    `.theme-dark, .theme-darker, .theme-midnight, .visual-refresh.theme-dark {\n${["--background-primary", "--background-secondary", "--background-secondary-alt", "--background-tertiary", "--background-floating", "--background-base-low", "--background-base-lower", "--background-base-lowest", "--background-surface-high", "--background-surface-higher", "--background-surface-highest", "--bg-overlay-chat", "--channeltextarea-background", "--chat-background-default", "--background-gradient-chat", "--bg-base-primary", "--bg-base-secondary", "--bg-base-tertiary"].map((n) => `  ${n}: #000 !important;`).join("\n")}\n  --background-mobile-primary: #000 !important;\n  --border-subtle: #1c1c1f !important;\n}`,
  ),
  styled(
    "nightLight",
    "Night Light",
    "Warms the screen's colours and can dim it, for late evenings.",
    {
      warmth: { type: "number", label: "Warmth (%)", default: 35, min: 0, max: 100 },
      brightness: { type: "number", label: "Brightness (%)", default: 90, min: 30, max: 100 },
    },
    (s) => `html { filter: sepia(${clamp(s.warmth, 0, 100, 35) / 100}) brightness(${clamp(s.brightness, 30, 100, 90) / 100}) !important; }`,
  ),
  styled(
    "colorFilter",
    "Colour Filter",
    "Adjusts saturation and contrast for the whole window. Set saturation to 0 for greyscale.",
    {
      saturation: { type: "number", label: "Saturation (%)", default: 0, min: 0, max: 200 },
      contrast: { type: "number", label: "Contrast (%)", default: 100, min: 50, max: 200 },
    },
    (s) => `html { filter: saturate(${clamp(s.saturation, 0, 200, 0) / 100}) contrast(${clamp(s.contrast, 50, 200, 100) / 100}) !important; }`,
  ),
  styled(
    "customScrollbars",
    "Custom Scrollbars",
    "Sets the width and colour of scrollbars.",
    {
      width: { type: "number", label: "Width (px)", default: 8, min: 2, max: 24 },
      color: { type: "string", label: "Colour", default: "#d2692f", placeholder: "#d2692f" },
    },
    (s) => `::-webkit-scrollbar { width: ${clamp(s.width, 2, 24, 8)}px !important; height: ${clamp(s.width, 2, 24, 8)}px !important; }\n::-webkit-scrollbar-thumb { background-color: ${cssValue(s.color, "#d2692f")} !important; border-radius: 99px !important; border: 0 !important; min-height: 40px !important; }\n::-webkit-scrollbar-track { background: transparent !important; border: 0 !important; }`,
  ),

  /* --------------------------------- effects -------------------------------- */
  style("noAnimations", "No Animations", "Turns off interface animations and transitions.", "*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; transition-duration: 0s !important; scroll-behavior: auto !important; }"),
  styled(
    "blurBackdrop",
    "Blur Behind Popups",
    "Blurs the app behind dialogs and image previews.",
    { amount: { type: "number", label: "Blur (px)", default: 6, min: 1, max: 30 } },
    (s) => `[class*="backdrop_"] { backdrop-filter: blur(${clamp(s.amount, 1, 30, 6)}px) !important; -webkit-backdrop-filter: blur(${clamp(s.amount, 1, 30, 6)}px) !important; }`,
  ),
  styled(
    "dimSidebars",
    "Dim Sidebars",
    "Fades the server, channel and member lists until you hover them, to keep attention on chat.",
    { opacity: { type: "number", label: "Opacity when idle (%)", default: 35, min: 5, max: 95, step: 5 } },
    (s) => `${GUILDS}, ${SIDEBAR} [class*="sidebarList"], ${MEMBERS} { opacity: ${clamp(s.opacity, 5, 95, 35) / 100} !important; transition: opacity .15s ease !important; }\n${GUILDS}:hover, ${SIDEBAR} [class*="sidebarList"]:hover, ${MEMBERS}:hover { opacity: 1 !important; }`,
  ),
  style("hoverHighlight", "Highlight Hovered Message", "Gives the message under the pointer a clearer background.", `${CHAT} ${ROW}:hover > div { background-color: var(--background-modifier-hover, rgba(78,80,88,.3)) !important; }`),
  style("zebraChat", "Striped Chat", "Shades every other message so long conversations are easier to follow.", `${CHAT} ${ROW}:nth-of-type(even) > div { background-color: rgba(128,128,128,.06) !important; }`),
  styled(
    "chatBubbles",
    "Chat Bubbles",
    "Puts message text in rounded bubbles.",
    { color: { type: "string", label: "Bubble colour", default: "rgba(128,128,128,.14)", placeholder: "rgba(128,128,128,.14)" } },
    (s) => `${CHAT} [class*="cozy_"] [class*="messageContent"]:not(:empty) { display: inline-block !important; background: ${cssValue(s.color, "rgba(128,128,128,.14)")} !important; border-radius: 4px 14px 14px 14px !important; padding: 6px 12px !important; margin-top: 2px !important; max-width: 85% !important; }`,
  ),
  style("boldUnreadChannels", "Bold Unread Channels", "Makes channels with unread messages stand out more in the channel list.", `${SIDEBAR} [class*="modeUnread"] [class*="name_"] { font-weight: 700 !important; color: var(--header-primary, #fff) !important; }\n${SIDEBAR} [class*="modeUnread"] [class*="unread_"] { width: 6px !important; height: 12px !important; }`),
  style(
    "focusedInput",
    "Highlight Message Box",
    "Outlines the message box while you are typing in it.",
    '[class*="channelTextArea"]:focus-within > [class*="scrollableContainer"], [class*="channelTextArea"]:focus-within { box-shadow: 0 0 0 2px var(--brand-500, #5865f2) !important; border-radius: 10px !important; }',
  ),
];
