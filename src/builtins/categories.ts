/**
 * The groups the Plugins page sorts plugins into, by what they do, in the order they are shown.
 * Every built-in plugin is listed in `PLUGIN_CATEGORY` below (a test checks this). Plugins
 * installed by the user can set `category` to one of these names to join a group.
 */
export const PLUGIN_CATEGORIES = [
  { name: "Appearance", description: "Colours, fonts, sizes and layout." },
  { name: "Declutter", description: "Hide buttons, badges and parts of Discord you don't use." },
  { name: "Quality of Life", description: "Shortcuts and small conveniences." },
  { name: "Chat", description: "More information and highlights on messages." },
  { name: "Media", description: "Images, videos and spoilers." },
  { name: "Writing and Sending", description: "Changes to the messages you send." },
  { name: "Notifications", description: "Notices about friends, messages and servers." },
  { name: "Filters and Muting", description: "Hide or quieten messages and pings from others." },
  { name: "Privacy and Safety", description: "Tracking, risky links and keeping your screen private." },
  { name: "Utilities", description: "Reminders, notes, stats and accessibility tools." },
] as const;

export type PluginCategory = (typeof PLUGIN_CATEGORIES)[number]["name"];

const groups: Record<PluginCategory, string[]> = {
  Appearance: [
    "customFont", "chatFontSize", "lineSpacing", "messageSpacing", "emojiSize", "underlineLinks", "plainUsernames", "boldUsernames",
    "selectionColor", "wrapCodeBlocks", "codeBlockHeight", "codeFontSize", "roundMedia", "chatWidth", "sidebarWidth", "memberListWidth",
    "autoHideMemberList", "autoHideServerList", "compactChannelList", "compactMemberList", "squareAvatars", "squareServerIcons",
    "sharpCorners", "uiScale", "accentColor", "mentionColor", "amoledBlack", "nightLight", "colorFilter", "customScrollbars",
    "noAnimations", "blurBackdrop", "dimSidebars", "hoverHighlight", "zebraChat", "chatBubbles", "boldUnreadChannels", "focusedInput",
    "channelColors", "userColors",
  ],
  Declutter: [
    "hideClutter", "hideAvatarDecorations", "hideProfileEffects", "hideNameplates", "hideServerTags", "hideRoleIcons", "hideBotTags",
    "hideNewMemberBadges", "hideProfileBadges", "calmSuperReactions", "hideEditedTag", "hideReactions", "hideStickers", "hideEmbeds",
    "hideReplyPreviews", "hideWaveButton", "hideChatAvatars", "hideDownloadButton", "hideDiscoverButton", "hideAddServerButton",
    "hideQuests", "hideLibrary", "hideHelpButton", "hideInboxButton", "hideThreadsButton", "hidePinsButton", "hideNotificationBell",
    "hideGifButton", "hideActivityButton", "hideSoundboardButton", "hideTopBanners", "hideChannelNotices", "hideServerBanner",
    "hideBoostBar", "hideServerShortcuts", "hideMutedChannels", "hideActivityPanel", "hideUnreadBadges", "hideUnreadPills",
    "hideNewMessagesBar", "hideJumpToPresent", "hideMentionJumpBars", "hideScrollbars", "hideChannelIcons", "hideTimestamps",
    "hideChannels", "hideServers", "hideDms",
  ],
  "Quality of Life": [
    "doubleClickEdit", "doubleClickReply", "altClickCopy", "charCounter", "disableSpellcheck", "quoteSelection", "pasteAsCode",
    "insertDate", "scrollButtons", "cleanTitle", "zenMode", "reloadKey", "fullscreenKey", "zoomKeys", "copyChannelLink", "noAutoIdle",
  ],
  Chat: [
    "alwaysShowTimestamps", "preciseTimestamps", "relativeTimestamps", "highlightOwn", "highlightFriends", "dimBots", "userTags",
    "userLocalTime", "accountAge", "collapseLongMessages", "highlightWords", "linkStatusBar", "keepDeleted",
  ],
  Media: [
    "blurMedia", "smallMedia", "revealSpoilersOnHover", "ctrlClickMedia", "showAltText", "videoSpeedKeys", "rememberVideoVolume",
    "loopVideos", "pictureInPicture",
  ],
  "Writing and Sending": [
    "textReplace", "embedFixer", "sendTimestamps", "silentMessages", "autoCapitalize", "typography", "tidyWhitespace",
    "inlineCalculator", "snippets", "suppressEmbeds", "mentionGuard", "calmCaps", "wordFilter", "messageAffixes", "textStyles",
    "escapeMarkdown", "linkShortcuts", "stripInvisible", "undoSend",
  ],
  Notifications: [
    "keywordNotify", "friendNotify", "gameNotify", "streamNotify", "relationshipNotify", "requestNotify", "voiceNotify", "voiceAlerts",
    "reactionNotify", "dmTypingNotify", "ghostPing", "deleteNotify", "editNotify", "userWatch", "channelWatch", "mentionHistory",
    "memberJoinNotify", "nicknameWatch", "threadNotify", "channelCreateNotify", "eventNotify", "connectionNotify", "dmNotify",
    "replyNotify", "customPingSound",
  ],
  "Filters and Muting": [
    "hideTypingIndicators", "ignoreEveryone", "noReplyPings", "hideBlocked", "localMute", "hideBots", "messageFilter",
    "hideSystemMessages", "embedBlocklist", "spoilerImages", "noStickersIncoming", "quietTyping",
  ],
  "Privacy and Safety": [
    "silentTyping", "noTrack", "scamWarning", "clearUrls", "maskedLinkWarning", "confirmLinks", "blurUnfocused", "privacyBlur", "panicKey",
  ],
  Utilities: ["readAloud", "voiceNarrator", "devtoolsKey", "cssHooks", "scratchpad", "reminders", "breakReminder", "birthdays", "dailyStats"],
};

/** Built-in plugin id → its group. */
export const PLUGIN_CATEGORY: Record<string, PluginCategory> = Object.fromEntries(
  Object.entries(groups).flatMap(([cat, ids]) => ids.map((id) => [id, cat as PluginCategory])),
);
