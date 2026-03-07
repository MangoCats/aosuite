// AO Timestamp — port of ao-types/src/timestamp.rs
// Unix seconds * 189,000,000, stored as 8-byte big-endian signed i64.

export const AO_MULTIPLIER = 189_000_000n;

/** Convert Unix seconds to AO timestamp value. */
export function fromUnixSeconds(seconds: bigint): bigint {
  return seconds * AO_MULTIPLIER;
}

/** Encode an AO timestamp as 8-byte big-endian. */
export function timestampToBytes(aoTimestamp: bigint): Uint8Array {
  const buf = new ArrayBuffer(8);
  const view = new DataView(buf);
  view.setBigInt64(0, aoTimestamp, false); // big-endian
  return new Uint8Array(buf);
}

/** Decode an AO timestamp from 8-byte big-endian at pos. Returns [value, 8]. */
export function timestampFromBytes(data: Uint8Array, pos: number): [bigint, number] {
  if (pos + 8 > data.length) throw new Error('unexpected end of timestamp data');
  const view = new DataView(data.buffer, data.byteOffset + pos, 8);
  return [view.getBigInt64(0, false), 8];
}

/** Get current Unix seconds as bigint. */
export function nowUnixSeconds(): bigint {
  return BigInt(Math.floor(Date.now() / 1000));
}
