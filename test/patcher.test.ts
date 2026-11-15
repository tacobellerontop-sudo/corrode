import { describe, expect, it, vi } from "vitest";
import { after, before, createScopedPatcher, getOriginal, instead, isPatched } from "../src/api/patcher";

const make = () => ({
  value: 2,
  mul(x: number) {
    return x * this.value;
  },
});

describe("patcher", () => {
  it("runs before/instead/after in order and keeps `this`", () => {
    const obj = make();
    const order: string[] = [];
    before(obj, "mul", (args) => {
      order.push("before");
      return [args[0] + 1];
    });
    instead(obj, "mul", (args, orig) => {
      order.push("instead");
      return orig(...args) * 10;
    });
    after(obj, "mul", (_args, res) => {
      order.push("after");
      return res + 1;
    });
    expect(obj.mul(1)).toBe((1 + 1) * 2 * 10 + 1);
    expect(order).toEqual(["before", "instead", "after"]);
  });

  it("restores the original once every patch is removed, in any order", () => {
    const obj = make();
    const original = obj.mul;
    const a = before(obj, "mul", () => {});
    const b = after(obj, "mul", () => {});
    expect(isPatched(obj.mul)).toBe(true);
    expect(getOriginal(obj.mul)).toBe(original);
    a();
    expect(isPatched(obj.mul)).toBe(true);
    b();
    expect(obj.mul).toBe(original);
    b(); // idempotent
  });

  it("isolates throwing callbacks", () => {
    const obj = make();
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    before(obj, "mul", () => {
      throw new Error("boom");
    });
    instead(obj, "mul", () => {
      throw new Error("boom");
    });
    after(obj, "mul", () => {
      throw new Error("boom");
    });
    expect(obj.mul(3)).toBe(6);
    expect(spy).toHaveBeenCalledTimes(3);
    spy.mockRestore();
  });

  it("chains multiple instead patches", () => {
    const obj = make();
    instead(obj, "mul", (args, orig) => orig(...args) + 1);
    instead(obj, "mul", (args, orig) => orig(...args) * 100);
    expect(obj.mul(1)).toBe((2 + 1) * 100);
  });

  it("scoped patcher removes all its patches", () => {
    const obj = make();
    const original = obj.mul;
    const scoped = createScopedPatcher("test");
    scoped.after(obj, "mul", () => 0);
    scoped.before(obj, "mul", () => {});
    expect(obj.mul(5)).toBe(0);
    scoped.unpatchAll();
    expect(obj.mul).toBe(original);
  });

  it("works on constructors", () => {
    const ns = {
      Foo: class {
        constructor(public x: number) {}
      },
    };
    after(ns, "Foo", (_args, res) => {
      res.patched = true;
    });
    const f: any = new ns.Foo(4);
    expect(f.x).toBe(4);
    expect(f.patched).toBe(true);
  });

  it("refuses non-functions", () => {
    expect(() => before({ a: 1 }, "a", () => {})).toThrow(TypeError);
  });
});
