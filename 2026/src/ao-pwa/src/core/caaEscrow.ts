// CAA (Conditional Assignment Agreement) escrow coordinator.
// Builds CAA DataItems and executes the ouroboros recording sequence.
//
// Protocol flow (2-chain swap A↔B):
// 1. Build fully-signed CAA with components for both chains
// 2. Submit CAA to chain A (order 0) → get recording proof A
// 3. Submit CAA + proof A to chain B (order 1) → get recording proof B (now BINDING)
// 4. Submit binding (both proofs) to chain A → finalized on A
// (Chain B is already finalized since it was last in order)

import {
  containerItem, vbcItem, bytesItem, toJson,
  type DataItem, type DataItemJson,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { encodeBigint, encodeRational, type Rational } from './bigint.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { hexToBytes } from './hex.ts';
import { RecorderClient, type CaaSubmitResponse, type CaaStatusResponse } from '../api/client.ts';

/** One chain's contribution to a CAA swap. */
export interface CaaChainInput {
  chainId: string;         // hex-encoded 32-byte chain ID
  recorderUrl: string;     // recorder base URL for this chain
  givers: CaaGiver[];
  receivers: CaaReceiver[];
  feeRate: { num: bigint; den: bigint };
}

export interface CaaGiver {
  seqId: bigint;
  amount: bigint;
  key: SigningKey;
}

export interface CaaReceiver {
  pubkey: Uint8Array;
  amount: bigint;
  key: SigningKey | null;   // null for external recipients
}

/** Result of a successful CAA execution. */
export interface CaaResult {
  caaHash: string;
  proofs: DataItemJson[];
}

/** Progress callback for UI updates during ouroboros sequence. */
export type CaaProgressCallback = (step: CaaProgressStep) => void;

export type CaaProgressStep =
  | { phase: 'building' }
  | { phase: 'submitting'; chainIndex: number; totalChains: number }
  | { phase: 'binding'; chainIndex: number; totalChains: number }
  | { phase: 'done' };

// ── CAA DataItem Construction ────────────────────────────────────────

/** Build an ASSIGNMENT DataItem for one CAA component (same structure as regular assignment). */
function buildComponentAssignment(
  givers: CaaGiver[],
  receivers: CaaReceiver[],
  feeRate: { num: bigint; den: bigint },
  deadline: bigint,  // AO timestamp
): DataItem {
  const participantCount = givers.length + receivers.length;
  const children: DataItem[] = [vbcItem(tc.LIST_SIZE, BigInt(participantCount))];

  for (const g of givers) {
    children.push(containerItem(tc.PARTICIPANT, [
      vbcItem(tc.SEQ_ID, g.seqId),
      bytesItem(tc.AMOUNT, encodeBigint(g.amount)),
    ]));
  }
  for (const r of receivers) {
    children.push(containerItem(tc.PARTICIPANT, [
      bytesItem(tc.ED25519_PUB, r.pubkey),
      bytesItem(tc.AMOUNT, encodeBigint(r.amount)),
    ]));
  }

  const bid: Rational = { num: feeRate.num, den: feeRate.den };
  children.push(bytesItem(tc.RECORDING_BID, encodeRational(bid)));
  children.push(bytesItem(tc.DEADLINE, timestampToBytes(deadline)));

  return containerItem(tc.ASSIGNMENT, children);
}

/** Sign a CAA component assignment (per-component AUTH_SIGs). */
async function signComponentAssignment(
  assignment: DataItem,
  givers: CaaGiver[],
  receivers: CaaReceiver[],
  baseUnixSecs: bigint,
): Promise<DataItem[]> {
  // signDataItem handles substituteSeparable internally — do not pre-substitute.
  const sigs: DataItem[] = [];
  const participantCount = givers.length + receivers.length;

  for (let i = 0; i < givers.length; i++) {
    const ts = fromUnixSeconds(baseUnixSecs + 1n + BigInt(i));
    const sig = await signDataItem(givers[i].key, assignment, ts);
    sigs.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      vbcItem(tc.PAGE_INDEX, BigInt(i)),
    ]));
  }

  for (let j = 0; j < receivers.length; j++) {
    if (!receivers[j].key) continue;
    const ts = fromUnixSeconds(baseUnixSecs + 1n + BigInt(participantCount) + BigInt(j));
    const sig = await signDataItem(receivers[j].key, assignment, ts);
    sigs.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      vbcItem(tc.PAGE_INDEX, BigInt(givers.length + j)),
    ]));
  }

  return sigs;
}

/** Build a CAA_COMPONENT DataItem for one chain. */
async function buildCaaComponent(
  chainInput: CaaChainInput,
  chainOrder: number,
  totalChains: number,
  deadline: bigint,     // AO timestamp
  bondAmount: bigint | null,  // coordinator bond (non-last chains)
  baseUnixSecs: bigint,
): Promise<DataItem> {
  const assignment = buildComponentAssignment(
    chainInput.givers, chainInput.receivers, chainInput.feeRate, deadline,
  );

  const children: DataItem[] = [
    bytesItem(tc.CHAIN_REF, hexToBytes(chainInput.chainId)),
    vbcItem(tc.CHAIN_ORDER, BigInt(chainOrder)),
  ];

  if (bondAmount !== null && chainOrder < totalChains - 1) {
    children.push(bytesItem(tc.COORDINATOR_BOND, encodeBigint(bondAmount)));
  }

  children.push(assignment);

  // Per-component signatures
  const sigs = await signComponentAssignment(
    assignment, chainInput.givers, chainInput.receivers, baseUnixSecs,
  );
  children.push(...sigs);

  return containerItem(tc.CAA_COMPONENT, children);
}

/** Collect all signing keys from all chain inputs (for overall CAA signatures). */
function collectAllKeys(chains: CaaChainInput[]): SigningKey[] {
  const keys: SigningKey[] = [];
  for (const chain of chains) {
    for (const g of chain.givers) keys.push(g.key);
    for (const r of chain.receivers) {
      if (r.key) keys.push(r.key);
    }
  }
  return keys;
}

/** Build and sign a complete CAA DataItem.
 *  Bond is computed as ceil(giverTotal / 10) per spec §9.1. */
export async function buildCaa(
  chains: CaaChainInput[],
  escrowSeconds: number,
): Promise<DataItem> {
  if (chains.length < 2) throw new Error('CAA requires at least 2 chains');
  if (chains.length > 8) throw new Error('CAA limited to 8 chains');
  if (escrowSeconds > 600) throw new Error('Escrow duration max 10 minutes (600s)');

  const baseUnixSecs = nowUnixSeconds();
  const deadlineSecs = baseUnixSecs + BigInt(escrowSeconds);
  const deadline = fromUnixSeconds(deadlineSecs);

  const caaChildren: DataItem[] = [
    bytesItem(tc.ESCROW_DEADLINE, timestampToBytes(deadline)),
    vbcItem(tc.LIST_SIZE, BigInt(chains.length)),
  ];

  // Build each component
  for (let i = 0; i < chains.length; i++) {
    // Coordinator bond: ceil(giverTotal / 10) on non-last chains (≥10% per spec §9.1)
    let bondAmount: bigint | null = null;
    if (i < chains.length - 1) {
      const giverTotal = chains[i].givers.reduce((sum, g) => sum + g.amount, 0n);
      bondAmount = (giverTotal + 9n) / 10n; // ceiling division
      if (bondAmount < 1n) bondAmount = 1n;
    }

    const component = await buildCaaComponent(
      chains[i], i, chains.length, deadline, bondAmount, baseUnixSecs,
    );
    caaChildren.push(component);
  }

  // Overall CAA signatures: all participants across all chains sign the canonical content.
  // Canonical content = everything built so far (ESCROW_DEADLINE, LIST_SIZE, CAA_COMPONENTs).
  // signDataItem handles substituteSeparable internally — pass raw canonical.
  const canonicalCaa = containerItem(tc.CAA, [...caaChildren]);

  const allKeys = collectAllKeys(chains);
  for (let k = 0; k < allKeys.length; k++) {
    const ts = fromUnixSeconds(baseUnixSecs + 100n + BigInt(k));
    const sig = await signDataItem(allKeys[k], canonicalCaa, ts);
    caaChildren.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      bytesItem(tc.ED25519_PUB, allKeys[k].publicKey),
    ]));
  }

  return containerItem(tc.CAA, caaChildren);
}

// ── Ouroboros Recording Sequence ─────────────────────────────────────

/** Execute the full CAA: build, submit to all chains in order, then bind.
 *  This is the main entry point for peer-to-peer escrow swaps. */
export async function executeCaa(
  chains: CaaChainInput[],
  escrowSeconds: number,
  onProgress?: CaaProgressCallback,
): Promise<CaaResult> {
  onProgress?.({ phase: 'building' });

  const caa = await buildCaa(chains, escrowSeconds);
  let caaJson = toJson(caa);

  const proofs: DataItemJson[] = [];
  let caaHash = '';

  // Ouroboros submission: submit to chain 0, then chain 1 with proof 0, etc.
  for (let i = 0; i < chains.length; i++) {
    onProgress?.({ phase: 'submitting', chainIndex: i, totalChains: chains.length });

    const client = new RecorderClient(chains[i].recorderUrl);

    // For chain i > 0, attach prior recording proofs to the CAA JSON.
    // The recorder expects proofs embedded in the top-level CAA DataItem.
    if (i > 0) {
      caaJson = attachProofsToCaaJson(caaJson, proofs);
    }

    const result: CaaSubmitResponse = await client.caaSubmit(chains[i].chainId, caaJson);
    caaHash = result.caa_hash;
    proofs.push(result.proof_json);
  }

  // Binding: submit all proofs to non-last chains
  for (let i = 0; i < chains.length - 1; i++) {
    onProgress?.({ phase: 'binding', chainIndex: i, totalChains: chains.length });

    const client = new RecorderClient(chains[i].recorderUrl);
    await client.caaBind(chains[i].chainId, caaHash, proofs);
  }

  onProgress?.({ phase: 'done' });
  return { caaHash, proofs };
}

/** Attach RECORDING_PROOF items to a CAA JSON structure. */
function attachProofsToCaaJson(caaJson: DataItemJson, proofs: DataItemJson[]): DataItemJson {
  // Recording proofs are added as top-level children of the CAA container
  const items = [...(caaJson.items ?? [])];

  // Remove any existing RECORDING_PROOF items (idempotent rebuild)
  const filtered = items.filter(item => item.code !== Number(tc.RECORDING_PROOF));

  // Add all current proofs
  for (const proof of proofs) {
    filtered.push(proof);
  }

  return { ...caaJson, items: filtered };
}

// ── Status Polling ───────────────────────────────────────────────────

/** CAA escrow status for display. */
export interface EscrowDisplayStatus {
  caaHash: string;
  chainId: string;
  chainSymbol: string;
  status: string;
  chainOrder: number;
  deadlineUnixSecs: number;
  blockHeight: number;
  hasProof: boolean;
}

/** Poll CAA status across all involved chains. */
export async function pollCaaStatus(
  caaHash: string,
  chains: { chainId: string; recorderUrl: string; symbol: string }[],
): Promise<EscrowDisplayStatus[]> {
  const results: EscrowDisplayStatus[] = [];

  const settled = await Promise.allSettled(
    chains.map(async (chain) => {
      const client = new RecorderClient(chain.recorderUrl);
      const status = await client.caaStatus(chain.chainId, caaHash);
      return { chain, status };
    }),
  );

  for (const r of settled) {
    if (r.status === 'fulfilled') {
      const { chain, status } = r.value;
      results.push({
        caaHash: status.caa_hash,
        chainId: chain.chainId,
        chainSymbol: chain.symbol,
        status: status.status,
        chainOrder: status.chain_order,
        deadlineUnixSecs: status.deadline,
        blockHeight: status.block_height,
        hasProof: status.has_proof,
      });
    }
  }

  return results;
}

/** Compute a simple overall status from per-chain statuses. */
export function overallCaaStatus(
  statuses: EscrowDisplayStatus[],
): 'unknown' | 'escrowed' | 'binding' | 'finalized' | 'expired' | 'partial' {
  if (statuses.length === 0) return 'unknown';
  const unique = new Set(statuses.map(s => s.status));
  if (unique.size === 1) return statuses[0].status as 'escrowed' | 'finalized' | 'expired';
  if (unique.has('finalized') && unique.has('escrowed')) return 'binding';
  if (unique.has('expired')) return 'expired';
  return 'partial';
}
