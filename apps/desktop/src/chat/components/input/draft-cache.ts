import type { JSONContent } from "@anlg/editor/chat";

const MAX_RETAINED_DRAFTS = 50;
const MAX_RETAINED_DRAFT_BYTES_PER_DRAFT = 16 * 1024 * 1024;
const MAX_RETAINED_DRAFT_BYTES = 32 * 1024 * 1024;

type DraftEntry = {
  content: JSONContent;
  byteSize: number;
};

type ActiveDraftEntry = {
  content: JSONContent | undefined;
  leases: number;
};

export type DraftRetentionFailure = {
  draftKey: string;
  reason: "cache-full" | "draft-too-large" | "older-draft-removed";
  removedDraftCount?: number;
};

export class DraftCache {
  private readonly retained = new Map<string, DraftEntry>();
  private readonly active = new Map<string, ActiveDraftEntry>();
  private retainedBytes = 0;

  constructor(
    private readonly limits = {
      maxCount: MAX_RETAINED_DRAFTS,
      maxDraftBytes: MAX_RETAINED_DRAFT_BYTES_PER_DRAFT,
      maxRetainedBytes: MAX_RETAINED_DRAFT_BYTES,
    },
  ) {}

  peek(draftKey: string) {
    const active = this.active.get(draftKey);
    if (active) {
      return active.content;
    }

    const retained = this.retained.get(draftKey);
    if (retained) {
      this.retained.delete(draftKey);
      this.retained.set(draftKey, retained);
    }
    return retained?.content;
  }

  acquire(draftKey: string, initialContent: JSONContent | undefined) {
    const active = this.active.get(draftKey);
    if (active) {
      active.leases += 1;
      return;
    }

    const retained = this.takeRetained(draftKey);
    const content = retained?.content ?? initialContent;
    this.active.set(draftKey, {
      content,
      leases: 1,
    });
  }

  update(draftKey: string, content: JSONContent | undefined) {
    const active = this.active.get(draftKey);
    if (!active) {
      return;
    }
    active.content = content;
  }

  release(draftKey: string): DraftRetentionFailure | undefined {
    const active = this.active.get(draftKey);
    if (!active) {
      return;
    }
    active.leases -= 1;
    if (active.leases > 0) {
      return;
    }

    this.active.delete(draftKey);
    if (active.content) {
      return this.retain(draftKey, active.content);
    }
  }

  delete(draftKey: string) {
    this.takeRetained(draftKey);
    const active = this.active.get(draftKey);
    if (active) {
      active.content = undefined;
    }
  }

  private retain(
    draftKey: string,
    content: JSONContent,
  ): DraftRetentionFailure | undefined {
    this.takeRetained(draftKey);
    const byteSize = measureDraftBytes(content, this.limits.maxDraftBytes);
    if (byteSize > this.limits.maxDraftBytes) {
      return { draftKey, reason: "draft-too-large" };
    }

    let removedDraftCount = 0;
    while (
      this.retained.size >= this.limits.maxCount ||
      this.retainedBytes + byteSize > this.limits.maxRetainedBytes
    ) {
      const oldestDraftKey = this.retained.keys().next().value;
      if (oldestDraftKey === undefined) {
        return { draftKey, reason: "cache-full" };
      }
      this.takeRetained(oldestDraftKey);
      removedDraftCount += 1;
    }

    const entry = { content, byteSize };
    this.retained.set(draftKey, entry);
    this.retainedBytes += entry.byteSize;
    if (removedDraftCount > 0) {
      return {
        draftKey,
        reason: "older-draft-removed",
        removedDraftCount,
      };
    }
  }

  private takeRetained(draftKey: string) {
    const entry = this.retained.get(draftKey);
    if (entry) {
      this.retained.delete(draftKey);
      this.retainedBytes -= entry.byteSize;
    }
    return entry;
  }
}

export function measureDraftBytes(
  content: JSONContent | undefined,
  maxBytes = Number.POSITIVE_INFINITY,
) {
  if (!content) {
    return 0;
  }

  let bytes = 0;
  const seen = new WeakSet<object>();

  const addBytes = (count: number) => {
    bytes += count;
    return bytes <= maxBytes;
  };

  const visit = (value: unknown, depth: number) => {
    if (bytes > maxBytes) {
      return;
    }
    if (depth > 256) {
      bytes = maxBytes + 1;
      return;
    }
    if (value === null || value === undefined) {
      addBytes(4);
    } else if (typeof value === "string") {
      addBytes(measureJsonStringBytes(value, maxBytes - bytes));
    } else if (typeof value === "number") {
      addBytes(Number.isFinite(value) ? String(value).length : 4);
    } else if (typeof value === "boolean") {
      addBytes(value ? 4 : 5);
    } else if (Array.isArray(value)) {
      if (seen.has(value)) {
        bytes = maxBytes + 1;
        return;
      }
      seen.add(value);
      addBytes(2);
      for (let index = 0; index < value.length; index += 1) {
        if (index > 0) {
          addBytes(1);
        }
        const entry = value[index];
        if (
          entry === undefined ||
          typeof entry === "function" ||
          typeof entry === "symbol"
        ) {
          addBytes(4);
        } else {
          visit(entry, depth + 1);
        }
        if (bytes > maxBytes) {
          break;
        }
      }
      seen.delete(value);
    } else if (typeof value === "object") {
      if (seen.has(value)) {
        bytes = maxBytes + 1;
        return;
      }
      seen.add(value);
      addBytes(2);
      let entryCount = 0;
      for (const key in value) {
        if (!Object.prototype.hasOwnProperty.call(value, key)) {
          continue;
        }
        const entry = (value as Record<string, unknown>)[key];
        if (
          entry === undefined ||
          typeof entry === "function" ||
          typeof entry === "symbol"
        ) {
          continue;
        }
        if (entryCount > 0) {
          addBytes(1);
        }
        addBytes(measureJsonStringBytes(key, maxBytes - bytes) + 1);
        visit(entry, depth + 1);
        entryCount += 1;
        if (bytes > maxBytes) {
          break;
        }
      }
      seen.delete(value);
    } else {
      bytes = maxBytes + 1;
    }
  };

  visit(content, 0);

  return bytes > maxBytes ? maxBytes + 1 : bytes;
}

function measureJsonStringBytes(value: string, maxBytes: number) {
  let bytes = 2;

  for (let index = 0; index < value.length && bytes <= maxBytes; index += 1) {
    const code = value.charCodeAt(index);
    if (code === 0x22 || code === 0x5c || code === 0x08 || code === 0x09) {
      bytes += 2;
    } else if (code === 0x0a || code === 0x0c || code === 0x0d) {
      bytes += 2;
    } else if (code < 0x20) {
      bytes += 6;
    } else if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (code >= 0xd800 && code <= 0xdbff) {
      const trailing = value.charCodeAt(index + 1);
      if (trailing >= 0xdc00 && trailing <= 0xdfff) {
        bytes += 4;
        index += 1;
      } else {
        bytes += 6;
      }
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      bytes += 6;
    } else {
      bytes += 3;
    }
  }

  return bytes;
}
