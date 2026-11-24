// The hosts a tunnel may reach by default: the clearnet names of the preset SMP relays
// (src/engine/presets.rs) and XFTP servers (src/xftp/servers.rs). ALLOWED_HOSTS adds more, and
// `*` there allows any public host.
import { parseList } from "./validate";

const smp = [4, 5, 6, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18, 19].map((n) => `smp${n}.simplex.im`);
const six = [1, 2, 3, 4, 5, 6];

export const PRESET_HOSTS: readonly string[] = [
  ...smp,
  ...six.map((n) => `smp${n}.simplexonflux.com`),
  ...six.map((n) => `xftp${n}.simplex.im`),
  ...six.map((n) => `xftp${n}.simplexonflux.com`),
];

/** The presets and the operator's ALLOWED_HOSTS: exact names, `*.example.com` or `*`. */
export function allowedHosts(value: string | undefined): string[] {
  return [...PRESET_HOSTS, ...parseList(value)];
}
