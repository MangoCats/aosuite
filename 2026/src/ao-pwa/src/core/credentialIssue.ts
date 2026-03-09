// Credential issuance and verification — N28.
// Implements face-to-face credential model: URL + SHA256 hash reference.
//
// Flow:
// 1. Issuer fetches credential document from URL
// 2. Compute SHA256 hash of document bytes
// 3. Build CREDENTIAL_REF DataItem (container with CREDENTIAL_URL + SHA256)
// 4. Attach as separable item to an assignment on vendor's chain
// 5. Consumer verifies by re-fetching URL and comparing hash
//
// Revocation: issuer updates the document or removes it from URL.
// Hash mismatch or fetch failure = credential no longer valid.

import { containerItem, bytesItem } from './dataitem.ts';
import type { DataItem, DataItemJson } from './dataitem.ts';
import * as tc from './typecodes.ts';
import { sha256 } from './hash.ts';
import { hexToBytes, bytesToHex } from './hex.ts';

/** Parsed credential reference extracted from chain data. */
export interface CredentialRefInfo {
  url: string;
  contentHash: string;      // hex-encoded SHA256
  blockHeight?: number;      // block where it was recorded
}

/** Result of verifying a credential reference. */
export type CredentialVerifyResult = 'verified' | 'mismatch' | 'unreachable';

// ── Building ────────────────────────────────────────────────────────

/** Fetch a document from a URL and compute its SHA256 hash. */
export async function fetchDocumentHash(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch credential document: ${response.status}`);
  }
  const bytes = new Uint8Array(await response.arrayBuffer());
  return sha256(bytes);
}

/** Build a CREDENTIAL_REF DataItem from a URL and content hash. */
export function buildCredentialRef(url: string, contentHash: Uint8Array): DataItem {
  if (contentHash.length !== 32) {
    throw new Error('Content hash must be 32 bytes (SHA256)');
  }
  const urlBytes = new TextEncoder().encode(url);
  return containerItem(tc.CREDENTIAL_REF, [
    bytesItem(tc.CREDENTIAL_URL, urlBytes),
    bytesItem(tc.SHA256, contentHash),
  ]);
}

/** Build a CREDENTIAL_REF by fetching the document and hashing it. */
export async function buildCredentialRefFromUrl(url: string): Promise<DataItem> {
  const hash = await fetchDocumentHash(url);
  return buildCredentialRef(url, hash);
}

// ── Verification ────────────────────────────────────────────────────

/** Verify a credential reference by fetching the URL and comparing hashes. */
export async function verifyCredential(
  cred: CredentialRefInfo,
): Promise<CredentialVerifyResult> {
  try {
    const hash = await fetchDocumentHash(cred.url);
    const expected = hexToBytes(cred.contentHash);
    if (hash.length !== expected.length) return 'mismatch';
    for (let i = 0; i < hash.length; i++) {
      if (hash[i] !== expected[i]) return 'mismatch';
    }
    return 'verified';
  } catch {
    return 'unreachable';
  }
}

// ── Parsing from chain data ─────────────────────────────────────────

/** Extract CREDENTIAL_REF items from a block's JSON representation. */
export function parseCredentialRefs(
  blocks: DataItemJson[],
): CredentialRefInfo[] {
  const refs: CredentialRefInfo[] = [];

  for (const block of blocks) {
    const height = findBlockHeight(block);
    walkForCredentials(block, height, refs);
  }

  return refs;
}

/** Recursively walk DataItemJson tree to find CREDENTIAL_REF containers. */
function walkForCredentials(
  item: DataItemJson,
  blockHeight: number | undefined,
  out: CredentialRefInfo[],
): void {
  if (item.code === Number(tc.CREDENTIAL_REF) && item.items) {
    const urlItem = item.items.find(c => c.code === Number(tc.CREDENTIAL_URL));
    const hashItem = item.items.find(c => c.code === Number(tc.SHA256));

    if (urlItem?.value != null && hashItem?.value != null) {
      // CREDENTIAL_URL value is hex-encoded UTF-8 in JSON wire format.
      const urlHex = String(urlItem.value);
      const urlBytes = hexToBytes(urlHex);
      const url = new TextDecoder().decode(urlBytes);

      out.push({
        url,
        contentHash: String(hashItem.value),
        blockHeight,
      });
    }
  }

  // Recurse into children
  if (item.items) {
    for (const child of item.items) {
      walkForCredentials(child, blockHeight, out);
    }
  }
}

/** Find BLOCK_HEIGHT or infer height from block structure. */
function findBlockHeight(block: DataItemJson): number | undefined {
  if (!block.items) return undefined;

  // Look for a FIRST_SEQ or other height indicator in block children
  // Block structure: BLOCK > BLOCK_SIGNED > BLOCK_CONTENTS
  for (const child of block.items) {
    if (child.code === Number(tc.BLOCK_SIGNED) && child.items) {
      for (const sc of child.items) {
        if (sc.code === Number(tc.BLOCK_CONTENTS) && sc.items) {
          const firstSeq = sc.items.find(c => c.code === Number(tc.FIRST_SEQ));
          if (firstSeq?.value != null) return Number(firstSeq.value);
        }
      }
    }
  }
  return undefined;
}

/** Deduplicate credential refs by URL (keep the latest block height). */
export function deduplicateCredentials(
  refs: CredentialRefInfo[],
): CredentialRefInfo[] {
  const byUrl = new Map<string, CredentialRefInfo>();
  for (const ref of refs) {
    const existing = byUrl.get(ref.url);
    if (!existing || (ref.blockHeight ?? 0) > (existing.blockHeight ?? 0)) {
      byUrl.set(ref.url, ref);
    }
  }
  return [...byUrl.values()];
}
