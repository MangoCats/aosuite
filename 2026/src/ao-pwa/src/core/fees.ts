// Recording fee calculation — port of ao-types/src/fees.rs
// fee_shares = ceil(data_bytes * fee_rate_num * shares_out / fee_rate_den)
// All arbitrary-precision integer arithmetic using native BigInt.

/** Ceiling division for positive integers: ceil(a / b) = (a + b - 1) / b. */
function ceilDiv(a: bigint, b: bigint): bigint {
  if (a <= 0n) return 0n;
  return (a + b - 1n) / b;
}

/** Compute recording fee in shares. */
export function recordingFee(
  dataBytes: bigint,
  feeRateNum: bigint,
  feeRateDen: bigint,
  sharesOut: bigint,
): bigint {
  const numerator = dataBytes * feeRateNum * sharesOut;
  return ceilDiv(numerator, feeRateDen);
}
