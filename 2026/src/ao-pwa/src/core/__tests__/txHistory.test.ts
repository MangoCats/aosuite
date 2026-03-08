import { describe, it, expect } from 'vitest';
import { parseBlocks, csvExporter, type TxRecord } from '../txHistory.ts';
import type { DataItemJson } from '../dataitem.ts';
import { encodeBigint } from '../bigint.ts';
import { bytesToHex } from '../hex.ts';
import { fromUnixSeconds, timestampToBytes, AO_MULTIPLIER } from '../timestamp.ts';

// Helper: encode a bigint amount as hex (the format used in block JSON)
function amountHex(value: bigint): string {
  return bytesToHex(encodeBigint(value));
}

// Helper: encode a timestamp as hex
function tsHex(unixSeconds: bigint): string {
  return bytesToHex(timestampToBytes(fromUnixSeconds(unixSeconds)));
}

// Fake pubkeys
const ALICE_PUB = 'aa'.repeat(32);
const BOB_PUB = 'bb'.repeat(32);
const CAROL_PUB = 'cc'.repeat(32);

/**
 * Build a minimal block JSON matching recorder output structure:
 * BLOCK > BLOCK_SIGNED > [BLOCK_CONTENTS > [PREV_HASH, FIRST_SEQ, SEQ_COUNT, LIST_SIZE, PAGE...], AUTH_SIG > [ED25519_SIG, TIMESTAMP, ED25519_PUB]]
 *   + SHA256
 */
function makeBlock(opts: {
  firstSeq: number;
  seqCount: number;
  timestamp: bigint;
  pages: DataItemJson[];
}): DataItemJson {
  return {
    type: 'BLOCK', code: 11,
    items: [
      {
        type: 'BLOCK_SIGNED', code: 12,
        items: [
          {
            type: 'BLOCK_CONTENTS', code: 13,
            items: [
              { type: 'PREV_HASH', code: 24, value: '00'.repeat(32) },
              { type: 'FIRST_SEQ', code: 25, value: opts.firstSeq },
              { type: 'SEQ_COUNT', code: 26, value: opts.seqCount },
              { type: 'LIST_SIZE', code: 27, value: opts.pages.length },
              ...opts.pages,
            ],
          },
          {
            type: 'AUTH_SIG', code: 30,
            items: [
              { type: 'ED25519_SIG', code: 2, value: '00'.repeat(64) },
              { type: 'TIMESTAMP', code: 5, value: tsHex(opts.timestamp) },
              { type: 'ED25519_PUB', code: 1, value: '00'.repeat(32) },
            ],
          },
        ],
      },
      { type: 'SHA256', code: 3, value: '00'.repeat(32) },
    ],
  };
}

function makePage(pageIndex: number, participants: DataItemJson[]): DataItemJson {
  return {
    type: 'PAGE', code: 14,
    items: [
      { type: 'PAGE_INDEX', code: 29, value: pageIndex },
      {
        type: 'AUTHORIZATION', code: 9,
        items: [
          {
            type: 'ASSIGNMENT', code: 8,
            items: [
              { type: 'LIST_SIZE', code: 27, value: participants.length },
              ...participants,
            ],
          },
        ],
      },
    ],
  };
}

function giverParticipant(seqId: number, amount: bigint): DataItemJson {
  return {
    type: 'PARTICIPANT', code: 10,
    items: [
      { type: 'SEQ_ID', code: 7, value: seqId },
      { type: 'AMOUNT', code: 6, value: amountHex(amount) },
    ],
  };
}

function receiverParticipant(pubkey: string, amount: bigint): DataItemJson {
  return {
    type: 'PARTICIPANT', code: 10,
    items: [
      { type: 'ED25519_PUB', code: 1, value: pubkey },
      { type: 'AMOUNT', code: 6, value: amountHex(amount) },
    ],
  };
}

describe('parseBlocks', () => {
  it('detects a received transaction', () => {
    const block = makeBlock({
      firstSeq: 2,
      seqCount: 1,
      timestamp: 1700000000n,
      pages: [makePage(0, [
        giverParticipant(1, 1000n),
        receiverParticipant(ALICE_PUB, 950n),
      ])],
    });

    const walletPubkeys = new Set([ALICE_PUB]);
    const walletSeqIds = new Map<number, string>();
    const records = parseBlocks([block], walletPubkeys, walletSeqIds, 1);

    expect(records).toHaveLength(1);
    expect(records[0].direction).toBe('received');
    expect(records[0].amount).toBe('950');
    expect(records[0].blockHeight).toBe(1);
    expect(records[0].receiverSeqId).toBe(2); // firstSeq
  });

  it('detects a sent transaction', () => {
    const block = makeBlock({
      firstSeq: 3,
      seqCount: 1,
      timestamp: 1700000000n,
      pages: [makePage(0, [
        giverParticipant(1, 1000n),
        receiverParticipant(BOB_PUB, 950n),
      ])],
    });

    const walletPubkeys = new Set([ALICE_PUB]);
    const walletSeqIds = new Map([[1, ALICE_PUB]]);
    const records = parseBlocks([block], walletPubkeys, walletSeqIds, 5);

    expect(records).toHaveLength(1);
    expect(records[0].direction).toBe('sent');
    expect(records[0].amount).toBe('1000');
    expect(records[0].counterparty).toBe(BOB_PUB);
    expect(records[0].blockHeight).toBe(5);
  });

  it('detects both sent and received in a self-transfer (change)', () => {
    const block = makeBlock({
      firstSeq: 10,
      seqCount: 2,
      timestamp: 1700000000n,
      pages: [makePage(0, [
        giverParticipant(5, 1000n),
        receiverParticipant(BOB_PUB, 700n),
        receiverParticipant(ALICE_PUB, 250n), // change back to self
      ])],
    });

    const walletPubkeys = new Set([ALICE_PUB]);
    const walletSeqIds = new Map([[5, ALICE_PUB]]);
    const records = parseBlocks([block], walletPubkeys, walletSeqIds, 10);

    // Should produce both a 'sent' and a 'received' record
    expect(records).toHaveLength(2);
    const sent = records.find(r => r.direction === 'sent')!;
    const received = records.find(r => r.direction === 'received')!;
    expect(sent.amount).toBe('1000');
    expect(sent.counterparty).toBe(BOB_PUB);
    expect(received.amount).toBe('250');
    expect(received.receiverSeqId).toBe(11); // secondReceiver: firstSeq(10) + 1
  });

  it('ignores blocks with no wallet involvement', () => {
    const block = makeBlock({
      firstSeq: 1,
      seqCount: 1,
      timestamp: 1700000000n,
      pages: [makePage(0, [
        giverParticipant(0, 500n),
        receiverParticipant(BOB_PUB, 480n),
      ])],
    });

    const walletPubkeys = new Set([ALICE_PUB]);
    const walletSeqIds = new Map<number, string>();
    const records = parseBlocks([block], walletPubkeys, walletSeqIds, 0);

    expect(records).toHaveLength(0);
  });

  it('handles multiple blocks', () => {
    const block1 = makeBlock({
      firstSeq: 1,
      seqCount: 1,
      timestamp: 1700000000n,
      pages: [makePage(0, [
        giverParticipant(0, 100n),
        receiverParticipant(ALICE_PUB, 90n),
      ])],
    });
    const block2 = makeBlock({
      firstSeq: 2,
      seqCount: 1,
      timestamp: 1700001000n,
      pages: [makePage(0, [
        giverParticipant(1, 90n),
        receiverParticipant(BOB_PUB, 80n),
      ])],
    });

    const walletPubkeys = new Set([ALICE_PUB]);
    const walletSeqIds = new Map([[1, ALICE_PUB]]);
    const records = parseBlocks([block1, block2], walletPubkeys, walletSeqIds, 5);

    expect(records).toHaveLength(2);
    expect(records[0].direction).toBe('received');
    expect(records[0].blockHeight).toBe(5);
    expect(records[1].direction).toBe('sent');
    expect(records[1].blockHeight).toBe(6);
  });

  it('extracts timestamp correctly', () => {
    const unixSecs = 1700000000n;
    const block = makeBlock({
      firstSeq: 1,
      seqCount: 1,
      timestamp: unixSecs,
      pages: [makePage(0, [
        giverParticipant(0, 100n),
        receiverParticipant(ALICE_PUB, 90n),
      ])],
    });

    const records = parseBlocks([block], new Set([ALICE_PUB]), new Map(), 0);
    expect(records[0].timestampMs).toBe(1700000000000);
  });

  it('detects blob attachment indicator', () => {
    const pageWithBlob: DataItemJson = {
      type: 'PAGE', code: 14,
      items: [
        { type: 'PAGE_INDEX', code: 29, value: 0 },
        {
          type: 'AUTHORIZATION', code: 9,
          items: [{
            type: 'ASSIGNMENT', code: 8,
            items: [
              { type: 'LIST_SIZE', code: 27, value: 2 },
              giverParticipant(0, 100n),
              receiverParticipant(ALICE_PUB, 90n),
              { type: 'DATA_BLOB', code: 33, value: 'deadbeef' },
            ],
          }],
        },
      ],
    };

    const block = makeBlock({
      firstSeq: 1, seqCount: 1, timestamp: 1700000000n,
      pages: [pageWithBlob],
    });

    const records = parseBlocks([block], new Set([ALICE_PUB]), new Map(), 0);
    expect(records[0].hasBlob).toBe(true);
  });
});

describe('csvExporter', () => {
  it('produces valid CSV header and rows', () => {
    const records: TxRecord[] = [
      {
        blockHeight: 5,
        pageIndex: 0,
        timestampMs: 1700000000000,
        direction: 'received',
        amount: '1000',
        counterparty: ALICE_PUB,
        hasBlob: false,
        giverSeqIds: [1],
        receiverSeqId: 2,
      },
    ];

    const csv = csvExporter.export(records, 'TEST', '1000000');
    const lines = csv.split('\n');
    expect(lines[0]).toBe('date,time,direction,amount_shares,amount_coins,counterparty,block_height,seq_id,has_blob');
    expect(lines).toHaveLength(2);

    const cols = lines[1].split(',');
    expect(cols[2]).toBe('received');
    expect(cols[3]).toBe('1000');
    expect(cols[6]).toBe('5');
    expect(cols[7]).toBe('2'); // receiverSeqId
    expect(cols[8]).toBe('false');
  });

  it('computes coin amount from shares and coin_count', () => {
    const records: TxRecord[] = [
      {
        blockHeight: 1, pageIndex: 0, timestampMs: 1700000000000,
        direction: 'sent', amount: '500000', counterparty: '',
        hasBlob: false, giverSeqIds: [1], receiverSeqId: null,
      },
    ];

    const csv = csvExporter.export(records, 'COIN', '1000000');
    const cols = csv.split('\n')[1].split(',');
    expect(cols[4]).toBe('0.500000'); // 500000 / 1000000
  });

  it('uses semicolons for multiple giver seq_ids in sent', () => {
    const records: TxRecord[] = [
      {
        blockHeight: 1, pageIndex: 0, timestampMs: 1700000000000,
        direction: 'sent', amount: '2000', counterparty: '',
        hasBlob: false, giverSeqIds: [3, 5], receiverSeqId: null,
      },
    ];

    const csv = csvExporter.export(records, 'X', '0');
    const cols = csv.split('\n')[1].split(',');
    expect(cols[7]).toBe('3;5');
  });
});
