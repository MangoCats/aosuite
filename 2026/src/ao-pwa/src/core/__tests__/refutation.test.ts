import { describe, it, expect } from 'vitest';
import { extractAssignment, computeAgreementHash } from '../refutation.ts';
import type { DataItemJson } from '../dataitem.ts';

// Build a minimal block JSON structure for testing.
function makeBlockJson(pageIndex: number, assignmentItems: DataItemJson[]): DataItemJson {
  return {
    type: 'BLOCK', code: 11,
    items: [{
      type: 'BLOCK_SIGNED', code: 12,
      items: [
        {
          type: 'BLOCK_CONTENTS', code: 13,
          items: [
            { type: 'FIRST_SEQ', code: 25, value: 0 },
            {
              type: 'PAGE', code: 14,
              items: [
                { type: 'PAGE_INDEX', code: 29, value: pageIndex },
                {
                  type: 'AUTHORIZATION', code: 9,
                  items: [{
                    type: 'ASSIGNMENT', code: 8,
                    items: assignmentItems,
                  }],
                },
              ],
            },
          ],
        },
      ],
    }],
  };
}

describe('extractAssignment', () => {
  it('extracts assignment at correct page index', () => {
    const assignmentItems: DataItemJson[] = [
      {
        type: 'PARTICIPANT', code: 10,
        items: [
          { type: 'SEQ_ID', code: 7, value: 0 },
          { type: 'AMOUNT', code: 6, value: '0a' },
        ],
      },
    ];
    const block = makeBlockJson(0, assignmentItems);
    const result = extractAssignment(block, 0);
    expect(result).not.toBeNull();
    expect(result!.type).toBe('ASSIGNMENT');
    expect(result!.items).toHaveLength(1);
  });

  it('returns null for wrong page index', () => {
    const block = makeBlockJson(0, []);
    expect(extractAssignment(block, 1)).toBeNull();
  });

  it('returns null for missing BLOCK_SIGNED', () => {
    const block: DataItemJson = { type: 'BLOCK', code: 11, items: [] };
    expect(extractAssignment(block, 0)).toBeNull();
  });

  it('returns null for missing AUTHORIZATION', () => {
    const block: DataItemJson = {
      type: 'BLOCK', code: 11,
      items: [{
        type: 'BLOCK_SIGNED', code: 12,
        items: [{
          type: 'BLOCK_CONTENTS', code: 13,
          items: [{
            type: 'PAGE', code: 14,
            items: [
              { type: 'PAGE_INDEX', code: 29, value: 0 },
            ],
          }],
        }],
      }],
    };
    expect(extractAssignment(block, 0)).toBeNull();
  });
});

describe('computeAgreementHash', () => {
  it('computes a 64-char hex hash', async () => {
    const assignment: DataItemJson = {
      type: 'ASSIGNMENT', code: 8,
      items: [
        {
          type: 'PARTICIPANT', code: 10,
          items: [
            { type: 'SEQ_ID', code: 7, value: 0 },
            { type: 'AMOUNT', code: 6, value: '0a' },
          ],
        },
      ],
    };
    const hash = await computeAgreementHash(assignment);
    expect(hash).toHaveLength(64);
    expect(hash).toMatch(/^[0-9a-f]{64}$/);
  });

  it('produces deterministic output', async () => {
    const assignment: DataItemJson = {
      type: 'ASSIGNMENT', code: 8,
      items: [
        {
          type: 'PARTICIPANT', code: 10,
          items: [
            { type: 'SEQ_ID', code: 7, value: 5 },
            { type: 'AMOUNT', code: 6, value: 'ff01' },
          ],
        },
      ],
    };
    const hash1 = await computeAgreementHash(assignment);
    const hash2 = await computeAgreementHash(assignment);
    expect(hash1).toBe(hash2);
  });

  it('different assignments produce different hashes', async () => {
    const a1: DataItemJson = {
      type: 'ASSIGNMENT', code: 8,
      items: [{
        type: 'PARTICIPANT', code: 10,
        items: [
          { type: 'SEQ_ID', code: 7, value: 0 },
          { type: 'AMOUNT', code: 6, value: '0a' },
        ],
      }],
    };
    const a2: DataItemJson = {
      type: 'ASSIGNMENT', code: 8,
      items: [{
        type: 'PARTICIPANT', code: 10,
        items: [
          { type: 'SEQ_ID', code: 7, value: 1 },
          { type: 'AMOUNT', code: 6, value: '0a' },
        ],
      }],
    };
    const h1 = await computeAgreementHash(a1);
    const h2 = await computeAgreementHash(a2);
    expect(h1).not.toBe(h2);
  });
});
