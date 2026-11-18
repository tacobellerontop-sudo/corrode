import { describe, expect, it } from "vitest";
import { builtinPlugins } from "../src/builtins";
import { PLUGIN_CATEGORIES, PLUGIN_CATEGORY } from "../src/builtins/categories";
import { matchesCombo, parseCombo, mapPlainText, snowflakeDate } from "../src/builtins/lib/helpers";
import * as send from "../src/builtins/sending";
import { findScamHost, isTrackingUrl, parseReminder, suspiciousHost } from "../src/builtins/utility";

describe("built-in plugin list", () => {
  it("has 204 plugins with unique ids and complete metadata", () => {
    expect(builtinPlugins.length).toBe(204);
    const ids = builtinPlugins.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    const names = builtinPlugins.map((p) => p.name);
    expect(new Set(names).size).toBe(names.length);
    for (const p of builtinPlugins) {
      expect(p.id, p.name).toMatch(/^[a-z][A-Za-z0-9]+$/);
      expect(p.name.length, p.id).toBeGreaterThan(2);
      expect(p.description?.length ?? 0, p.id).toBeGreaterThan(15);
      expect(p.enabledByDefault ?? false, p.id).toBe(false);
      expect(!!(p.css || p.start || p.flux || p.patches), p.id).toBe(true);
      for (const [key, def] of Object.entries(p.settings ?? {}) as [string, any][]) {
        expect(def.label, `${p.id}.${key}`).toBeTruthy();
        expect(def.default, `${p.id}.${key}`).not.toBeUndefined();
        if (def.type === "select") expect(def.options.some((o: any) => o.value === def.default), `${p.id}.${key}`).toBe(true);
      }
    }
  });

  it("puts every built-in plugin in exactly one group", () => {
    const names = PLUGIN_CATEGORIES.map((c) => c.name as string);
    for (const p of builtinPlugins) {
      expect(PLUGIN_CATEGORY[p.id], p.id).toBeTruthy();
      expect(names, p.id).toContain(p.category);
    }
    expect(Object.keys(PLUGIN_CATEGORY).sort()).toEqual(builtinPlugins.map((p) => p.id).sort());
  });

  it("builds valid CSS from default settings, and survives hostile setting values", () => {
    for (const p of builtinPlugins) {
      if (typeof p.css !== "function") continue;
      const defaults = Object.fromEntries((Object.entries(p.settings ?? {}) as [string, any][]).map(([k, d]) => [k, d.default]));
      const css = p.css(defaults as any);
      expect(typeof css, p.id).toBe("string");
      expect((css.match(/{/g) ?? []).length, p.id).toBe((css.match(/}/g) ?? []).length);
      const hostile = Object.fromEntries(Object.keys(defaults).map((k) => [k, "red; } body { display: none } x {"]));
      const bad = p.css(hostile as any);
      expect(bad, p.id).not.toContain("display: none } x");
      expect((bad.match(/{/g) ?? []).length, p.id).toBe((bad.match(/}/g) ?? []).length);
    }
  });
});

describe("helpers", () => {
  it("parses and matches shortcuts", () => {
    expect(parseCombo("Ctrl+Shift+K")).toEqual({ mod: true, shift: true, alt: false, key: "k" });
    expect(parseCombo("f5")).toEqual({ mod: false, shift: false, alt: false, key: "f5" });
    expect(parseCombo("")).toBeUndefined();
    expect(parseCombo("ctrl+a+b")).toBeUndefined();
    const ev = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
    expect(matchesCombo(ev({ key: "K", code: "KeyK", ctrlKey: true, shiftKey: true }), parseCombo("ctrl+shift+k")!)).toBe(true);
    expect(matchesCombo(ev({ key: "k", code: "KeyK", ctrlKey: true }), parseCombo("ctrl+shift+k")!)).toBe(false);
    expect(matchesCombo(ev({ key: "F5", code: "F5" }), parseCombo("f5")!)).toBe(true);
    expect(matchesCombo(ev({ key: "π", code: "KeyP", altKey: true }), parseCombo("alt+p")!)).toBe(true);
  });
  it("leaves code, links, mentions and emoji alone", () => {
    const up = (s: string) => mapPlainText(s, (t) => t.toUpperCase());
    expect(up("hi `code` https://a.b/c <@123> <:e:1> ```x\ny``` end")).toBe("HI `code` https://a.b/c <@123> <:e:1> ```x\ny``` END");
  });
  it("reads snowflake dates", () => {
    expect(snowflakeDate("175928847299117063").toISOString()).toBe("2016-04-30T11:18:25.796Z");
  });
});

describe("sending transforms", () => {
  it("clears tracking parameters only", () => {
    expect(send.clearUrls("see https://a.com/p?utm_source=x&id=5&fbclid=abc.")).toBe("see https://a.com/p?id=5.");
    expect(send.clearUrls("https://youtu.be/abc?si=xyz&t=10")).toBe("https://youtu.be/abc?t=10");
    expect(send.clearUrls("https://example.com/?si=keep")).toBe("https://example.com/?si=keep");
    expect(send.clearUrls("https://x.com/u/status/1?s=20&t=abc")).toBe("https://x.com/u/status/1");
    expect(send.clearUrls("no links here")).toBe("no links here");
  });
  it("applies replacement rules to whole words, outside code", () => {
    const rules = send.parseRules("teh => the\n/colou?r/i => COLOR\nbad line\n => x");
    expect(rules).toHaveLength(2);
    expect(send.applyRules("Teh color of `teh` other", rules)).toBe("the COLOR of `teh` other");
    expect(send.applyRules("tehran", rules)).toBe("tehran");
  });
  it("swaps hosts", () => {
    expect(send.swapHosts("https://www.x.com/a/status/1 and https://example.com/x.com", { "x.com": "fxtwitter.com" })).toBe("https://fxtwitter.com/a/status/1 and https://example.com/x.com");
  });
  it("expands timestamps", () => {
    const now = new Date(2026, 0, 5, 9, 0, 0);
    const unix = (d: Date) => Math.floor(d.getTime() / 1000);
    expect(send.expandTimestamps("at [[14:30]]", now)).toBe(`at <t:${unix(new Date(2026, 0, 5, 14, 30))}:t>`);
    expect(send.expandTimestamps("[[2:30pm]]", now)).toBe(`<t:${unix(new Date(2026, 0, 5, 14, 30))}:t>`);
    expect(send.expandTimestamps("[[12am]]", now)).toBe(`<t:${unix(new Date(2026, 0, 5, 0, 0))}:t>`);
    expect(send.expandTimestamps("[[2026-03-01 18:00]]", now)).toBe(`<t:${unix(new Date(2026, 2, 1, 18, 0))}:f>`);
    expect(send.expandTimestamps("[[in 1h 30m]]", now)).toBe(`<t:${unix(now) + 5400}:R>`);
    expect(send.expandTimestamps("[[now]]", now)).toBe(`<t:${unix(now)}:t>`);
    for (const keep of ["[[hello]]", "[[25:00]]", "[[7]]", "[[in 3 parsecs]]", "[[13pm]]"]) expect(send.expandTimestamps(keep, now)).toBe(keep);
  });
  it("capitalises sentences", () => {
    expect(send.capitalize("hello there. how are you? fine")).toBe("Hello there. How are you? Fine");
    expect(send.capitalize("https://a.com is up")).toBe("https://a.com is up");
    expect(send.capitalize("  ok\nnext line")).toBe("  Ok\nNext line");
    expect(send.capitalize("see <:e:1>this")).toBe("See <:e:1>this");
  });
  it("converts typography both ways", () => {
    expect(send.smartTypography(`"hi" it's... a -- b`)).toBe("“hi” it’s… a — b");
    expect(send.plainTypography("“hi” it’s… a — b")).toBe(`"hi" it's... a - b`);
  });
  it("tidies whitespace but not code blocks", () => {
    expect(send.tidyWhitespace("  a  b \n\n\n\nc ```x  y\n\n\n\nz``` ")).toBe("a b\n\nc ```x  y\n\n\n\nz```");
  });
  it("calculates", () => {
    expect(send.calculate("2+2*3")).toBe(8);
    expect(send.calculate("(2+2)*3")).toBe(12);
    expect(send.calculate("-2^2")).toBe(4);
    expect(send.calculate("2^3^2")).toBe(512);
    expect(send.calculate("10 % 4 / 2")).toBe(1);
    expect(send.calculate("1/0")).toBeUndefined();
    expect(send.calculate("alert(1)")).toBeUndefined();
    expect(send.calculate("2+")).toBeUndefined();
    expect(send.expandCalculations("total {{0.1+0.2}} and {{nope}}")).toBe("total 0.3 and {{nope}}");
  });
  it("expands snippets and link shortcuts", () => {
    const snippets = send.parsePairs("shrug = ¯\\_(ツ)_/¯\ntwo = a\\nb");
    expect(send.expandSnippets("well ;shrug and ;unknown", snippets)).toBe("well ¯\\_(ツ)_/¯ and ;unknown");
    expect(send.expandSnippets(";two", snippets)).toBe("a\nb");
    const links = send.parsePairs("gh = https://github.com/%s\nyt = https://youtube.com/results?q=%s");
    expect(send.expandLinkShortcuts("see gh:user/repo or yt:a&b, not http://x", links)).toBe("see https://github.com/user/repo or https://youtube.com/results?q=a%26b, not http://x");
  });
  it("suppresses embeds", () => {
    expect(send.suppressEmbeds("a https://a.com/x. b <https://b.com> `https://c.com`")).toBe("a <https://a.com/x>. b <https://b.com> `https://c.com`");
    expect(send.suppressEmbeds("https://a.com https://sub.b.com", ["b.com"])).toBe("https://a.com <https://sub.b.com>");
  });
  it("calms caps, censors, styles, escapes and strips", () => {
    expect(send.calmCaps("THIS IS VERY LOUD")).toBe("This is very loud");
    expect(send.calmCaps("NASA is ok")).toBe("NASA is ok");
    expect(send.censor("darn that Darn thing, darning", ["darn"], "spoiler")).toBe("||darn|| that ||Darn|| thing, darning");
    expect(send.censor("darn", ["darn"], "stars")).toBe("d***");
    expect(send.applyTextStyle("!upper hello <@1>")).toBe("HELLO <@1>");
    expect(send.applyTextStyle("!nope hello")).toBe("!nope hello");
    expect(send.applyTextStyle("hello")).toBe("hello");
    expect(send.escapeMarkdown("*a* _b_")).toBe("\\*a\\* \\_b\\_");
    expect(send.stripInvisible("a​b﻿c")).toBe("abc");
  });
});

describe("utility helpers", () => {
  it("spots tracking requests", () => {
    expect(isTrackingUrl("https://discord.com/api/v9/science")).toBe(true);
    expect(isTrackingUrl("https://o64374.ingest.sentry.io/api/1/envelope/")).toBe(true);
    expect(isTrackingUrl("https://discord.com/api/v9/channels/1/messages")).toBe(false);
  });
  it("spots imitation hosts but not the real ones", () => {
    for (const bad of ["discorcl.com", "dlscord-nitro.com", "discord-gift.ru", "steamcommunlty.com", "free-nitro.xyz", "www.disc0rd.gift"]) expect(suspiciousHost(bad), bad).toBe(true);
    for (const ok of ["discord.com", "cdn.discordapp.com", "discord.gg", "steamcommunity.com", "example.com", "github.com", "media.discordapp.net"]) expect(suspiciousHost(ok), ok).toBe(false);
    expect(findScamHost("get it at https://dlscord-gift.com/x now")).toBe("dlscord-gift.com");
    expect(findScamHost("https://discord.com/invite/x")).toBeUndefined();
  });
  it("parses reminders", () => {
    expect(parseReminder("!remind 1h30m take a break")).toEqual({ seconds: 5400, text: "take a break" });
    expect(parseReminder("!remind 10 s")).toEqual({ seconds: 10, text: "Reminder" });
    expect(parseReminder("!remind soon")).toBeUndefined();
    expect(parseReminder("hello")).toBeUndefined();
  });
});
