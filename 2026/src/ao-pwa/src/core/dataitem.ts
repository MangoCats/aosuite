// DataItem — port of ao-types/src/dataitem.rs
// Recursive TLV structure: type code (signed VBC) + payload.

import { encodeSigned, decodeSigned, encodeUnsigned, decodeUnsigned } from './vbc.ts';
import { sizeCategory, typeName } from './typecodes.ts';
import { bytesToHex, hexToBytes, concatBytes } from './hex.ts';

export type DataValue =
  | { kind: 'bytes'; data: Uint8Array }
  | { kind: 'vbcValue'; value: bigint }
  | { kind: 'container'; children: DataItem[] };

export interface DataItem {
  typeCode: bigint;
  value: DataValue;
}

// --- Constructors ---

export function bytesItem(typeCode: bigint, data: Uint8Array): DataItem {
  return { typeCode, value: { kind: 'bytes', data } };
}

export function vbcItem(typeCode: bigint, value: bigint): DataItem {
  return { typeCode, value: { kind: 'vbcValue', value } };
}

export function containerItem(typeCode: bigint, children: DataItem[]): DataItem {
  return { typeCode, value: { kind: 'container', children } };
}

// --- Binary encoding ---

export function encodeDataItem(item: DataItem): Uint8Array {
  const typeVbc = encodeSigned(item.typeCode);

  switch (item.value.kind) {
    case 'bytes': {
      const cat = sizeCategory(item.typeCode);
      if (cat?.kind === 'fixed') {
        return concatBytes(typeVbc, item.value.data);
      }
      // Variable: size prefix + data
      const sizeVbc = encodeUnsigned(BigInt(item.value.data.length));
      return concatBytes(typeVbc, sizeVbc, item.value.data);
    }
    case 'vbcValue': {
      const valVbc = encodeUnsigned(item.value.value);
      return concatBytes(typeVbc, valVbc);
    }
    case 'container': {
      const childParts = item.value.children.map(encodeDataItem);
      let childLen = 0;
      for (const p of childParts) childLen += p.length;
      const childBuf = new Uint8Array(childLen);
      let offset = 0;
      for (const p of childParts) {
        childBuf.set(p, offset);
        offset += p.length;
      }
      const sizeVbc = encodeUnsigned(BigInt(childBuf.length));
      return concatBytes(typeVbc, sizeVbc, childBuf);
    }
  }
}

/** Encode a DataItem to its binary form. */
export function toBytes(item: DataItem): Uint8Array {
  return encodeDataItem(item);
}

// --- Binary decoding ---

export function decodeDataItem(data: Uint8Array, pos: number): [DataItem, number] {
  const [typeCode, tcLen] = decodeSigned(data, pos);
  let offset = pos + tcLen;

  const cat = sizeCategory(typeCode);
  if (!cat) throw new Error(`unknown type code ${typeCode}`);

  switch (cat.kind) {
    case 'fixed': {
      const n = cat.size;
      if (offset + n > data.length) throw new Error('unexpected end of data');
      const bytes = data.slice(offset, offset + n);
      offset += n;
      return [bytesItem(typeCode, bytes), offset - pos];
    }
    case 'variable': {
      const [size, sizeLen] = decodeUnsigned(data, offset);
      offset += sizeLen;
      const n = Number(size);
      if (offset + n > data.length) throw new Error('unexpected end of data');
      const bytes = data.slice(offset, offset + n);
      offset += n;
      return [bytesItem(typeCode, bytes), offset - pos];
    }
    case 'vbcValue': {
      const [value, vbcLen] = decodeUnsigned(data, offset);
      offset += vbcLen;
      return [vbcItem(typeCode, value), offset - pos];
    }
    case 'container': {
      const [size, sizeLen] = decodeUnsigned(data, offset);
      offset += sizeLen;
      const containerEnd = offset + Number(size);
      if (containerEnd > data.length) throw new Error('unexpected end of data');
      const children: DataItem[] = [];
      while (offset < containerEnd) {
        const [child, childLen] = decodeDataItem(data, offset);
        offset += childLen;
        children.push(child);
      }
      if (offset !== containerEnd) throw new Error(`container has trailing bytes`);
      return [containerItem(typeCode, children), offset - pos];
    }
  }
}

/** Decode a DataItem from a complete byte buffer. */
export function fromBytes(data: Uint8Array): DataItem {
  const [item, consumed] = decodeDataItem(data, 0);
  if (consumed !== data.length) throw new Error(`trailing bytes: ${data.length - consumed}`);
  return item;
}

// --- JSON serialization (matching ao-types/src/json.rs) ---

export interface DataItemJson {
  type: string;
  code: number;
  value?: string | number;
  items?: DataItemJson[];
}

export function toJson(item: DataItem): DataItemJson {
  const name = typeName(item.typeCode) ?? 'UNKNOWN';
  const code = Number(item.typeCode);

  switch (item.value.kind) {
    case 'bytes':
      return { type: name, code, value: bytesToHex(item.value.data) };
    case 'vbcValue':
      return { type: name, code, value: Number(item.value.value) };
    case 'container':
      return { type: name, code, items: item.value.children.map(toJson) };
  }
}

export function fromJson(json: DataItemJson): DataItem {
  const code = BigInt(json.code);
  const cat = sizeCategory(code);
  if (!cat) throw new Error(`unknown type code ${json.code}`);

  switch (cat.kind) {
    case 'fixed':
    case 'variable': {
      if (typeof json.value !== 'string') throw new Error(`expected hex string for code ${json.code}`);
      return bytesItem(code, hexToBytes(json.value));
    }
    case 'vbcValue': {
      if (typeof json.value !== 'number') throw new Error(`expected number for VBC code ${json.code}`);
      return vbcItem(code, BigInt(json.value));
    }
    case 'container': {
      if (!json.items) throw new Error(`expected items array for container code ${json.code}`);
      return containerItem(code, json.items.map(fromJson));
    }
  }
}

// --- Accessors ---

export function children(item: DataItem): DataItem[] {
  return item.value.kind === 'container' ? item.value.children : [];
}

export function findChild(item: DataItem, typeCode: bigint): DataItem | undefined {
  return children(item).find(c => c.typeCode === typeCode);
}

export function asBytes(item: DataItem): Uint8Array | undefined {
  return item.value.kind === 'bytes' ? item.value.data : undefined;
}

export function asVbcValue(item: DataItem): bigint | undefined {
  return item.value.kind === 'vbcValue' ? item.value.value : undefined;
}
