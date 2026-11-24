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
