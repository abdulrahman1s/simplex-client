// Admits one client's tunnels: open-tunnel cap and an exact new-tunnel rate. One SQLite-backed
// Durable Object per hashed IP; its storage is deleted once it holds nothing, and an alarm
// clears leases that were not renewed and starts that left the window.
import { DurableObject } from "cloudflare:workers";
import {
  admit,
  emptyState,
  nextExpiry,
  prune,
  release,
  renew,
  type Admission,
  type CounterRules,
  type CounterState,
} from "./leases";

const KEY = "state";

export class TunnelCounter extends DurableObject {
  async admit(id: string, rules: CounterRules): Promise<Admission> {
    return this.update((s, now) => admit(s, id, rules, now));
  }

  async renew(id: string, leaseMs: number): Promise<void> {
    await this.update((s, now) => renew(s, id, now, leaseMs));
  }

  async release(id: string): Promise<void> {
    await this.update((s) => release(s, id));
  }

  async alarm(): Promise<void> {
    await this.update(() => undefined);
  }

  private async update<T>(change: (s: CounterState, now: number) => T): Promise<T> {
    const kv = this.ctx.storage.kv;
    const now = Date.now();
    const state = kv.get<CounterState>(KEY) ?? emptyState();
    prune(state, now);
    const result = change(state, now);
    const next = nextExpiry(state);
    if (next === null) {
      await this.ctx.storage.deleteAlarm();
      await this.ctx.storage.deleteAll();
    } else {
      kv.put(KEY, state);
      await this.ctx.storage.setAlarm(next);
    }
    return result;
  }
}
