// The per-IP limits fail closed: when the rate limiter or the counter errors, the tunnel is
// refused, so an outage there cannot turn the limits off.
import type { Admission } from "./leases";

/** The fast first gate before the WebSocket opens; true lets the upgrade through. */
export async function firstGate(limit: () => Promise<{ success: boolean }>): Promise<boolean> {
  try {
    return (await limit()).success;
  } catch {
    return false;
  }
}

/**
 * The counter's answer, or `rate_limited` when it cannot give one. `heldLease` is true when
 * the tunnel holds a lease to give back: when admitted, and when the counter errored, since it
 * may have taken the lease before failing. Releasing an id it never took changes nothing.
 */
export type Admitted<T> = { admission: "ok"; value: T } | { admission: Exclude<Admission, "ok"> };

/**
 * Starts `lookup` and asks the counter at once, so a tunnel waits for the slower of the two
 * rather than both in turn. The counter's refusal wins over whatever the lookup found, so a
 * limited client always reads `rate_limited` or `too_many_tunnels` and backs off for that.
 * `lookup` must not reject: an unawaited rejection after a refusal would go unhandled.
 */
export async function admitWhile<T>(
  admit: () => Promise<Admission>,
  lookup: () => Promise<T>,
): Promise<Admitted<T>> {
  const found = lookup();
  const admission = await admit();
  if (admission !== "ok") return { admission };
  return { admission, value: await found };
}

export async function askCounter(
  admit: () => Promise<Admission>,
): Promise<{ admission: Admission; heldLease: boolean }> {
  try {
    const admission = await admit();
    return { admission, heldLease: admission === "ok" };
  } catch {
    return { admission: "rate_limited", heldLease: true };
  }
}
