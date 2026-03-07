// BigInt and Rational encoding — port of ao-types/src/bigint.rs
// BigInt: unsigned VBC byte count + two's-complement big-endian magnitude.
// Rational: TotalSize(VBC) + NumSize(VBC) + Numerator(TC BE) + Denominator(TC BE).

import { encodeUnsigned, decodeUnsigned } from './vbc.ts';
import { hexToBytes, concatBytes } from './hex.ts';

export class BigIntEncodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BigIntEncodeError';
  }
}

// --- BigInt encoding ---

/** Convert a bigint to minimal two's-complement big-endian bytes. */
export function bigintToTcBe(value: bigint): Uint8Array {
  if (value === 0n) return new Uint8Array(0);

  // Work with magnitude, then apply two's complement
  const negative = value < 0n;
  let mag = negative ? -value : value;

  // Convert to bytes (big-endian)
  const hexStr = mag.toString(16);
  const padded = hexStr.length % 2 === 0 ? hexStr : '0' + hexStr;
  const bytes = hexToBytes(padded);

  if (negative) {
    // Two's complement: invert all bits, add 1
    const result = new Uint8Array(bytes.length);
    let carry = 1;
    for (let i = bytes.length - 1; i >= 0; i--) {
      const inverted = (~bytes[i] & 0xff) + carry;
      result[i] = inverted & 0xff;
      carry = inverted >> 8;
    }
    // If high bit is 0, we need a 0xff prefix to stay negative
    if ((result[0] & 0x80) === 0) {
      const extended = new Uint8Array(result.length + 1);
      extended[0] = 0xff;
      extended.set(result, 1);
      return extended;
    }
    return result;
  } else {
    // Positive: if high bit is set, prepend 0x00
    if ((bytes[0] & 0x80) !== 0) {
      const extended = new Uint8Array(bytes.length + 1);
      extended[0] = 0x00;
      extended.set(bytes, 1);
      return extended;
    }
    return bytes;
  }
}

/** Convert two's-complement big-endian bytes back to bigint. */
export function tcBeToBigint(bytes: Uint8Array): bigint {
  if (bytes.length === 0) return 0n;

  const negative = (bytes[0] & 0x80) !== 0;

  if (negative) {
    // Two's complement: invert bits, add 1, negate
    const inverted = new Uint8Array(bytes.length);
    let carry = 1;
    for (let i = bytes.length - 1; i >= 0; i--) {
      const val = (~bytes[i] & 0xff) + carry;
      inverted[i] = val & 0xff;
      carry = val >> 8;
    }
    let mag = 0n;
    for (const b of inverted) {
      mag = (mag << 8n) | BigInt(b);
    }
    return -mag;
  } else {
    let value = 0n;
    for (const b of bytes) {
      value = (value << 8n) | BigInt(b);
    }
    return value;
  }
}

/** Encode a bigint as VBC byte count + two's-complement big-endian bytes. */
export function encodeBigint(value: bigint): Uint8Array {
  if (value === 0n) return encodeUnsigned(0n);
  const magnitude = bigintToTcBe(value);
  const sizeVbc = encodeUnsigned(BigInt(magnitude.length));
  return concatBytes(sizeVbc, magnitude);
}

/** Decode a bigint from data starting at pos. Returns [value, bytesConsumed]. */
export function decodeBigint(data: Uint8Array, pos: number): [bigint, number] {
  const [byteCount, vbcLen] = decodeUnsigned(data, pos);
  const count = Number(byteCount);
  const magStart = pos + vbcLen;
  const magEnd = magStart + count;

  if (magEnd > data.length) throw new BigIntEncodeError('unexpected end of bigint data');
  if (count === 0) return [0n, vbcLen];

  const magnitude = data.slice(magStart, magEnd);

  // Check minimality
  if (count > 1) {
    const first = magnitude[0];
    const second = magnitude[1];
    if (first === 0x00 && second < 0x80) throw new BigIntEncodeError('non-minimal bigint encoding');
    if (first === 0xff && second >= 0x80) throw new BigIntEncodeError('non-minimal bigint encoding');
  }

  return [tcBeToBigint(magnitude), vbcLen + count];
}

// --- Rational encoding ---

export interface Rational {
  num: bigint;
  den: bigint;
}

/** Encode a rational as TotalSize + NumSize + Numerator + Denominator. */
export function encodeRational(value: Rational): Uint8Array {
  const numBytes = value.num === 0n ? new Uint8Array(0) : bigintToTcBe(value.num);
  const denomBytes = bigintToTcBe(value.den);
  const numSizeVbc = encodeUnsigned(BigInt(numBytes.length));
  const total = numSizeVbc.length + numBytes.length + denomBytes.length;
  const totalVbc = encodeUnsigned(BigInt(total));
  return concatBytes(totalVbc, numSizeVbc, numBytes, denomBytes);
}

/** Decode a rational from data starting at pos. Returns [value, bytesConsumed]. */
export function decodeRational(data: Uint8Array, pos: number): [Rational, number] {
  const [totalSize, totalVbcLen] = decodeUnsigned(data, pos);
  const total = Number(totalSize);
  const contentStart = pos + totalVbcLen;

  if (contentStart + total > data.length) throw new BigIntEncodeError('unexpected end of rational data');

  const [numSize, numSizeVbcLen] = decodeUnsigned(data, contentStart);
  const nSize = Number(numSize);
  const numStart = contentStart + numSizeVbcLen;
  const numEnd = numStart + nSize;
  const denomSize = total - numSizeVbcLen - nSize;

  if (denomSize <= 0) throw new BigIntEncodeError('rational denominator must be positive');

  const denomStart = numEnd;
  const denomEnd = denomStart + denomSize;
  if (denomEnd > data.length) throw new BigIntEncodeError('unexpected end of rational data');

  const num = nSize === 0 ? 0n : tcBeToBigint(data.slice(numStart, numEnd));
  const den = tcBeToBigint(data.slice(denomStart, denomEnd));
  if (den <= 0n) throw new BigIntEncodeError('rational denominator must be positive');

  return [{ num, den }, totalVbcLen + total];
}
