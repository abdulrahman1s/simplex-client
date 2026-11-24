// Counts one client's open tunnels. One SQLite-backed Durable Object per hashed IP; its storage
// is deleted when the last tunnel closes, and an alarm clears leases whose release was lost.
import { DurableObject } from "cloudflare:workers";
import { acquire, prune, type LeaseStore } from "./leases";

export class TunnelCounter extends DurableObject {
  async acquire(id: string, max: number, ttlMs: number): Promise<boolean> {
    const now = Date.now();
    const ok = acquire(this.leases(), id, max, now + ttlMs, now);
    await this.tidy(now);
    return ok;
  }

  async release(id: string): Promise<void> {
    this.ctx.storage.kv.delete(id);
    await this.tidy(Date.now());
  }

  async alarm(): Promise<void> {
    await this.tidy(Date.now());
  }

  private async tidy(now: number): Promise<void> {
    const next = prune(this.leases(), now);
    if (next === null) {
      await this.ctx.storage.deleteAlarm();
      await this.ctx.storage.deleteAll();
    } else {
      await this.ctx.storage.setAlarm(next);
    }
  }

  private leases(): LeaseStore {
    const kv = this.ctx.storage.kv;
    return {
      entries: () => kv.list<number>(),
      put: (id, expiresAt) => kv.put(id, expiresAt),
      delete: (id) => void kv.delete(id),
    };
  }
}
