// Refutation — computes agreement hashes and submits refutations.
// NOTE: The recorder accepts unsigned refutations (bare agreement hash).
// A future protocol revision may require signed REFUTATION DataItems
// to prevent unauthorized refutation of agreements by third parties.

import { RecorderClient } from '../api/client.ts';
import type { DataItemJson } from './dataitem.ts';
import { fromJson, toBytes } from './dataitem.ts';
import { sha256 } from './hash.ts';
import { bytesToHex } from './hex.ts';

/** Find a child item by type name in a DataItemJson. */
function findChild(item: DataItemJson, typeName: string): DataItemJson | undefined {
  return item.items?.find(c => c.type === typeName);
}

/** Find all children with a given type name. */
function findChildren(item: DataItemJson, typeName: string): DataItemJson[] {
  return item.items?.filter(c => c.type === typeName) ?? [];
}

/**
 * Extract the ASSIGNMENT DataItemJson from a block at a given page index.
 * Returns null if the assignment cannot be found.
 */
export function extractAssignment(
  blockJson: DataItemJson,
  pageIndex: number,
): DataItemJson | null {
  const blockSigned = findChild(blockJson, 'BLOCK_SIGNED');
  if (!blockSigned) return null;
  const blockContents = findChild(blockSigned, 'BLOCK_CONTENTS');
  if (!blockContents) return null;

  const pages = findChildren(blockContents, 'PAGE');
  for (const page of pages) {
    const idxItem = findChild(page, 'PAGE_INDEX');
    if (!idxItem || typeof idxItem.value !== 'number') continue;
    if (idxItem.value !== pageIndex) continue;

    const authorization = findChild(page, 'AUTHORIZATION');
    if (!authorization) return null;
    return findChild(authorization, 'ASSIGNMENT') ?? null;
  }
  return null;
}

/**
 * Compute the agreement hash (SHA2-256) of an ASSIGNMENT DataItemJson.
 * This matches the hash format expected by POST /chain/{id}/refute.
 */
export async function computeAgreementHash(assignmentJson: DataItemJson): Promise<string> {
  const dataItem = fromJson(assignmentJson);
  const bytes = toBytes(dataItem);
  const hash = await sha256(bytes);
  return bytesToHex(hash);
}

/**
 * Fetch a block, extract the assignment at the given page, compute its hash,
 * and submit the refutation to the recorder.
 */
export async function refuteTransaction(
  recorderUrl: string,
  chainId: string,
  blockHeight: number,
  pageIndex: number,
): Promise<string> {
  const client = new RecorderClient(recorderUrl);

  // Fetch the specific block
  const blocks = await client.getBlocks(chainId, blockHeight, blockHeight + 1);
  if (blocks.length === 0) {
    throw new Error(`Block ${blockHeight} not found`);
  }

  const assignment = extractAssignment(blocks[0], pageIndex);
  if (!assignment) {
    throw new Error(`Assignment not found at block ${blockHeight}, page ${pageIndex}`);
  }

  const hash = await computeAgreementHash(assignment);
  await client.refute(chainId, hash);
  return hash;
}
