// ClientHello records built byte for byte like simplex-client's src/tls/handshake/client_hello.rs.

export interface HelloOptions {
  alpn?: string[];
  /** Adds a server_name extension, which simplex-client never sends. */
  sni?: string;
  random?: Uint8Array;
  keyShare?: Uint8Array;
  extraExtensions?: Uint8Array[];
}

const u16 = (n: number) => [(n >> 8) & 0xff, n & 0xff];
const u24 = (n: number) => [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
const cat = (...parts: (Uint8Array | number[])[]) => Uint8Array.from(parts.flatMap((p) => [...p]));

export function extension(type: number, data: Uint8Array | number[]): Uint8Array {
  return cat(u16(type), u16(data.length), data);
}

export function rustClientHello(o: HelloOptions = {}): Uint8Array {
  const alpn = o.alpn ?? ["smp/1"];
  const random = o.random ?? new Uint8Array(32).fill(7);
  const key = o.keyShare ?? new Uint8Array(32).fill(9);
  const u16s = (items: number[]) => [...u16(items.length * 2), ...items.flatMap(u16)];
  const exts: Uint8Array[] = [
    extension(43, [2, 3, 4]),
    extension(10, u16s([0x001d])),
    extension(13, u16s([0x0808, 0x0807])),
    extension(51, cat(u16(36), u16(0x001d), u16(32), key)),
  ];
  if (o.sni !== undefined) {
    const name = new TextEncoder().encode(o.sni);
    exts.push(extension(0, cat(u16(name.length + 3), [0], u16(name.length), name)));
  }
  if (alpn.length > 0) {
    const names = alpn.flatMap((p) => [p.length, ...new TextEncoder().encode(p)]);
    exts.push(extension(16, cat(u16(names.length), names)));
  }
  exts.push(...(o.extraExtensions ?? []));
  const extBytes = cat(...exts);
  const body = cat([3, 3], random, [0], [0, 2], u16(0x1303), [1, 0], u16(extBytes.length), extBytes);
  const message = cat([1], u24(body.length), body);
  return cat([0x16, 3, 1], u16(message.length), message);
}

/** Lists the extension types of a ClientHello record, to check what a captured one carries. */
export function extensionTypes(record: Uint8Array): number[] {
  let p = 5 + 4 + 2 + 32;
  p += 1 + record[p];
  p += 2 + ((record[p] << 8) | record[p + 1]);
  p += 1 + record[p];
  const end = p + 2 + ((record[p] << 8) | record[p + 1]);
  p += 2;
  const types: number[] = [];
  while (p < end) {
    types.push((record[p] << 8) | record[p + 1]);
    p += 4 + ((record[p + 2] << 8) | record[p + 3]);
  }
  return types;
}
