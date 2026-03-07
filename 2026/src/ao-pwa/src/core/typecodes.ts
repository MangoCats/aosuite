// Type code constants — port of ao-types/src/typecode.rs

// Core inseparable types (|code| 1-31)
export const ED25519_PUB = 1n;
export const ED25519_SIG = 2n;
export const SHA256 = 3n;
export const BLAKE3 = 4n;
export const TIMESTAMP = 5n;
export const AMOUNT = 6n;
export const SEQ_ID = 7n;
export const ASSIGNMENT = 8n;
export const AUTHORIZATION = 9n;
export const PARTICIPANT = 10n;
export const BLOCK = 11n;
export const BLOCK_SIGNED = 12n;
export const BLOCK_CONTENTS = 13n;
export const PAGE = 14n;
export const GENESIS = 15n;
export const RECORDING_BID = 16n;
export const DEADLINE = 17n;
export const COIN_COUNT = 18n;
export const FEE_RATE = 19n;
export const EXPIRY_PERIOD = 20n;
export const CHAIN_SYMBOL = 21n;
export const PROTOCOL_VER = 22n;
export const SHARES_OUT = 23n;
export const PREV_HASH = 24n;
export const FIRST_SEQ = 25n;
export const SEQ_COUNT = 26n;
export const LIST_SIZE = 27n;
export const REFUTATION = 28n;
export const PAGE_INDEX = 29n;
export const AUTH_SIG = 30n;
export const REFERRAL_FEE = 31n;

// Negative type codes
export const EXPIRY_MODE = -1n;
export const TAX_PARAMS = -2n;

// Separable types (|code| 32-63)
export const NOTE = 32n;
export const DATA_BLOB = 33n;
export const DESCRIPTION = 34n;
export const ICON = 35n;
export const VENDOR_PROFILE = 36n;
export const EXCHANGE_LISTING = 37n;
export const CREDENTIAL_REF = 38n;
export const CREDENTIAL_URL = 39n;

// Inseparable types, second band (|code| 64-95)
export const VALIDATOR_ATTESTATION = 64n;
export const VALIDATED_HEIGHT = 65n;
export const ROLLED_HASH = 66n;
export const ANCHOR_REF = 67n;
export const ANCHOR_TIMESTAMP = 68n;

export type SizeCategory =
  | { kind: 'fixed'; size: number }
  | { kind: 'variable' }
  | { kind: 'vbcValue' }
  | { kind: 'container' };

const fixed = (size: number): SizeCategory => ({ kind: 'fixed', size });
const variable: SizeCategory = { kind: 'variable' };
const vbcValue: SizeCategory = { kind: 'vbcValue' };
const container: SizeCategory = { kind: 'container' };

const SIZE_CATEGORIES = new Map<bigint, SizeCategory>([
  [ED25519_PUB, fixed(32)],
  [ED25519_SIG, fixed(64)],
  [SHA256, fixed(32)],
  [BLAKE3, fixed(32)],
  [TIMESTAMP, fixed(8)],
  [DEADLINE, fixed(8)],
  [EXPIRY_PERIOD, fixed(8)],
  [PREV_HASH, fixed(32)],
  [ROLLED_HASH, fixed(32)],
  [ANCHOR_TIMESTAMP, fixed(8)],

  [AMOUNT, variable], [RECORDING_BID, variable], [COIN_COUNT, variable],
  [FEE_RATE, variable], [CHAIN_SYMBOL, variable], [SHARES_OUT, variable],
  [REFERRAL_FEE, variable],
  [NOTE, variable], [DATA_BLOB, variable], [DESCRIPTION, variable], [ICON, variable],
  [CREDENTIAL_URL, variable], [ANCHOR_REF, variable],

  [SEQ_ID, vbcValue], [PROTOCOL_VER, vbcValue], [FIRST_SEQ, vbcValue],
  [SEQ_COUNT, vbcValue], [LIST_SIZE, vbcValue], [PAGE_INDEX, vbcValue],
  [EXPIRY_MODE, vbcValue], [VALIDATED_HEIGHT, vbcValue],

  [ASSIGNMENT, container], [AUTHORIZATION, container], [PARTICIPANT, container],
  [BLOCK, container], [BLOCK_SIGNED, container], [BLOCK_CONTENTS, container],
  [PAGE, container], [GENESIS, container], [REFUTATION, container],
  [AUTH_SIG, container], [TAX_PARAMS, container],
  [VENDOR_PROFILE, container], [EXCHANGE_LISTING, container],
  [CREDENTIAL_REF, container], [VALIDATOR_ATTESTATION, container],
]);

export function sizeCategory(code: bigint): SizeCategory | undefined {
  return SIZE_CATEGORIES.get(code);
}

/** Check if a type code's item is separable: |code| & 0x20 != 0 */
export function isSeparable(code: bigint): boolean {
  const abs = code < 0n ? -code : code;
  return (abs & 0x20n) !== 0n;
}

const TYPE_NAMES = new Map<bigint, string>([
  [ED25519_PUB, 'ED25519_PUB'], [ED25519_SIG, 'ED25519_SIG'],
  [SHA256, 'SHA256'], [BLAKE3, 'BLAKE3'], [TIMESTAMP, 'TIMESTAMP'],
  [AMOUNT, 'AMOUNT'], [SEQ_ID, 'SEQ_ID'], [ASSIGNMENT, 'ASSIGNMENT'],
  [AUTHORIZATION, 'AUTHORIZATION'], [PARTICIPANT, 'PARTICIPANT'],
  [BLOCK, 'BLOCK'], [BLOCK_SIGNED, 'BLOCK_SIGNED'],
  [BLOCK_CONTENTS, 'BLOCK_CONTENTS'], [PAGE, 'PAGE'], [GENESIS, 'GENESIS'],
  [RECORDING_BID, 'RECORDING_BID'], [DEADLINE, 'DEADLINE'],
  [COIN_COUNT, 'COIN_COUNT'], [FEE_RATE, 'FEE_RATE'],
  [EXPIRY_PERIOD, 'EXPIRY_PERIOD'], [CHAIN_SYMBOL, 'CHAIN_SYMBOL'],
  [PROTOCOL_VER, 'PROTOCOL_VER'], [SHARES_OUT, 'SHARES_OUT'],
  [PREV_HASH, 'PREV_HASH'], [FIRST_SEQ, 'FIRST_SEQ'],
  [SEQ_COUNT, 'SEQ_COUNT'], [LIST_SIZE, 'LIST_SIZE'],
  [REFUTATION, 'REFUTATION'], [PAGE_INDEX, 'PAGE_INDEX'],
  [AUTH_SIG, 'AUTH_SIG'], [REFERRAL_FEE, 'REFERRAL_FEE'],
  [EXPIRY_MODE, 'EXPIRY_MODE'],
  [TAX_PARAMS, 'TAX_PARAMS'], [NOTE, 'NOTE'], [DATA_BLOB, 'DATA_BLOB'],
  [DESCRIPTION, 'DESCRIPTION'], [ICON, 'ICON'],
  [VENDOR_PROFILE, 'VENDOR_PROFILE'], [EXCHANGE_LISTING, 'EXCHANGE_LISTING'],
  [CREDENTIAL_REF, 'CREDENTIAL_REF'], [CREDENTIAL_URL, 'CREDENTIAL_URL'],
  [VALIDATOR_ATTESTATION, 'VALIDATOR_ATTESTATION'],
  [VALIDATED_HEIGHT, 'VALIDATED_HEIGHT'], [ROLLED_HASH, 'ROLLED_HASH'],
  [ANCHOR_REF, 'ANCHOR_REF'], [ANCHOR_TIMESTAMP, 'ANCHOR_TIMESTAMP'],
]);

export function typeName(code: bigint): string | undefined {
  return TYPE_NAMES.get(code);
}
