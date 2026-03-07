// Variable Byte Coding (VBC) — port of ao-types/src/vbc.rs
// Unsigned: 7 data bits per byte (bits 0-6), bit 7 = continuation, LSB-first.
// Signed: zigzag mapping. n >= 0 → wire = n<<1; n < 0 → wire = ((-n)<<1)|1.

const MAX_VBC_BYTES = 10;

export class VbcError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'VbcError';
  }
}

// --- Unsigned VBC ---

/** Encode an unsigned bigint as VBC bytes, return Uint8Array. */
export function encodeUnsigned(value: bigint): Uint8Array {
  if (value < 0n) throw new VbcError('encodeUnsigned: negative value');
  const bytes: number[] = [];
  let v = value;
  do {
    let byte = Number(v & 0x7fn);
    v >>= 7n;
    if (v > 0n) byte |= 0x80;
    bytes.push(byte);
  } while (v > 0n);
  return new Uint8Array(bytes);
}

/** Decode an unsigned VBC value from data starting at pos.
 *  Returns [value, bytesConsumed]. */
export function decodeUnsigned(data: Uint8Array, pos: number): [bigint, number] {
  let value = 0n;
  let shift = 0n;
  let i = pos;
  for (;;) {
    if (i >= data.length) throw new VbcError('unexpected end of VBC data');
    if (i - pos >= MAX_VBC_BYTES) throw new VbcError('VBC value overflow (>10 bytes)');
    const byte = data[i];
    value |= BigInt(byte & 0x7f) << shift;
    i++;
    if ((byte & 0x80) === 0) break;
    shift += 7n;
  }
  return [value, i - pos];
}

// --- Signed VBC ---

/** Encode a signed bigint as VBC bytes (zigzag encoding). */
export function encodeSigned(value: bigint): Uint8Array {
  const wire = value >= 0n ? value << 1n : ((-value) << 1n) | 1n;
  return encodeUnsigned(wire);
}

/** Decode a signed VBC value from data starting at pos.
 *  Returns [value, bytesConsumed]. */
export function decodeSigned(data: Uint8Array, pos: number): [bigint, number] {
  const [wire, consumed] = decodeUnsigned(data, pos);
  if (wire === 1n) throw new VbcError('negative zero is invalid in signed VBC');
  const magnitude = wire >> 1n;
  const value = (wire & 1n) === 0n ? magnitude : -magnitude;
  return [value, consumed];
}
