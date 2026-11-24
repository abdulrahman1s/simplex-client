// The deployment's bindings, vars and secrets (see wrangler.toml and README.md).
import type { TunnelCounter } from "./counter";
import type { LimitVars } from "./limits";

export interface Env extends LimitVars {
  ALLOWED_PORTS?: string;
  ALLOWED_HOSTS?: string;
  ALLOWED_ORIGINS?: string;
  /** Secret. When set, the first frame must carry the same `token`. */
  TUNNEL_TOKEN?: string;
  /** Secret. Salt for hashing client IPs. */
  IP_SALT?: string;
  TUNNEL_RL?: RateLimit;
  TUNNELS?: DurableObjectNamespace<TunnelCounter>;
}
