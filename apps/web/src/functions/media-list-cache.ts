export type ExpiringCacheEntry<T> = {
  expiresAt: number;
  value: T;
};

export class BoundedInFlightRequests<T> {
  private readonly active = new Set<Promise<T>>();
  private readonly byKey = new Map<string, Promise<T>>();
  private readonly maxConcurrent: number;

  constructor(maxConcurrent: number) {
    if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
      throw new Error("maxConcurrent must be a positive integer");
    }
    this.maxConcurrent = maxConcurrent;
  }

  get(key: string) {
    return this.byKey.get(key);
  }

  getOrStart(key: string, load: () => Promise<T>): Promise<T> | undefined {
    const existing = this.byKey.get(key);
    if (existing) return existing;
    if (this.active.size >= this.maxConcurrent) return undefined;

    let request: Promise<T>;
    request = Promise.resolve()
      .then(load)
      .finally(() => {
        this.active.delete(request);
        if (this.byKey.get(key) === request) {
          this.byKey.delete(key);
        }
      });
    this.byKey.set(key, request);
    this.active.add(request);
    return request;
  }

  clearKeys() {
    this.byKey.clear();
  }

  deleteWhere(predicate: (key: string) => boolean) {
    for (const key of this.byKey.keys()) {
      if (predicate(key)) {
        this.byKey.delete(key);
      }
    }
  }
}

export function getFreshCacheValue<T>(
  cache: Map<string, ExpiringCacheEntry<T>>,
  key: string,
  now: number,
  maxEntries: number,
) {
  pruneExpiringCache(cache, now, maxEntries);
  const entry = cache.get(key);
  if (!entry) return undefined;

  cache.delete(key);
  cache.set(key, entry);
  return entry.value;
}

export function setExpiringCacheValue<T>(
  cache: Map<string, ExpiringCacheEntry<T>>,
  key: string,
  value: T,
  now: number,
  ttlMs: number,
  maxEntries: number,
) {
  cache.delete(key);
  cache.set(key, { expiresAt: now + ttlMs, value });
  pruneExpiringCache(cache, now, maxEntries);
}

function pruneExpiringCache<T>(
  cache: Map<string, ExpiringCacheEntry<T>>,
  now: number,
  maxEntries: number,
) {
  for (const [key, entry] of cache) {
    if (entry.expiresAt <= now) {
      cache.delete(key);
    }
  }

  while (cache.size > maxEntries) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey === undefined) break;
    cache.delete(oldestKey);
  }
}
