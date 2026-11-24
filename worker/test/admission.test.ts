import { describe, expect, it } from "vitest";
import { admitWhile } from "../src/admission";
import type { Admission } from "../src/leases";

function deferred<T>(): { promise: Promise<T>; resolve: (v: T) => void } {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe("admitWhile", () => {
  it("starts the lookup before the counter answers", async () => {
    const counter = deferred<Admission>();
    let looked = false;
    const result = admitWhile(
      () => counter.promise,
      async () => {
        looked = true;
        return "93.184.215.14";
      },
    );
    await Promise.resolve();
    expect(looked).toBe(true);
    counter.resolve("ok");
    expect(await result).toEqual({ admission: "ok", value: "93.184.215.14" });
  });

  it("answers with the counter's refusal even when the lookup failed first", async () => {
    const counter = deferred<Admission>();
    const result = admitWhile(
      () => counter.promise,
      async () => null,
    );
    counter.resolve("too_many_tunnels");
    expect(await result).toEqual({ admission: "too_many_tunnels" });
  });

  it("does not wait for a slow lookup once the counter refuses", async () => {
    const lookup = deferred<string>();
    const result = await admitWhile(
      async (): Promise<Admission> => "rate_limited",
      () => lookup.promise,
    );
    expect(result).toEqual({ admission: "rate_limited" });
  });
});
