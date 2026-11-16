import { describe, expect, it } from "vitest";
import { buildCss, cleanSrc, fileName, newState, normalize, PARTS, PRESETS, readState } from "../src/themeCreator";
import { parseMeta } from "../src/themes";

describe("theme creator", () => {
  it("round-trips its choices through the CSS file", () => {
    const s = newState();
    s.name = "Night Owl ✨";
    s.author = "LX";
    s.colors.chat = "#123456";
    s.font = "Georgia, serif";
    s.background = { src: "media/bg-2.png", kind: "image", fit: "tile", blur: 4, dim: 20, panels: 60 };
    const css = buildCss(s);
    expect(css).toContain("--background-primary: rgba(18, 52, 86, 0.6) !important;");
    expect(css).toContain("@background media/bg-2.png");
    expect(parseMeta(css)).toMatchObject({ name: "Night Owl ✨", author: "LX" });
    expect(readState(css)).toEqual(normalize(s));
    expect((css.match(/{/g) ?? []).length).toBe((css.match(/}/g) ?? []).length);
  });

  it("has a full palette in every preset", () => {
    for (const p of PRESETS) for (const part of PARTS) expect(p.colors[part.id], `${p.name} ${part.id}`).toMatch(/^#[0-9a-f]{6}$/);
  });

  it("can't be broken out of by names, fonts or links", () => {
    const css = buildCss(normalize({ name: "x */ body { display: none } /*", font: "a; } body { display:none } x {", background: { src: "https://e.com/a.png\") } body { x" } }));
    expect(css).not.toMatch(/\*\/ body \{ display: none/);
    expect(css).not.toContain("display:none }");
    expect((css.match(/{/g) ?? []).length).toBe((css.match(/}/g) ?? []).length);
    expect(cleanSrc("javascript:alert(1)")).toBe("");
    expect(cleanSrc("media/../x.png")).toBe("");
    expect(readState("/* not a creator theme */")).toBeUndefined();
  });

  it("names files safely", () => {
    expect(fileName("My: Theme?")).toBe("My Theme.theme.css");
    expect(fileName("..")).toBe("theme.theme.css");
  });
});
