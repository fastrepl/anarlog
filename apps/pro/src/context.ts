import { CacheableMemory } from "@cacheable/memory";
import type { MiddlewareHandler } from "hono";

declare module "hono" {
  interface ContextVariableMap {
    cache: CacheableMemory;
    cacheSet: <T>(key: string, value: T, ttl?: string | number) => void;
    cacheGet: <T>(key: string) => T | undefined;
    cacheHas: (key: string) => boolean;
    cacheDelete: (key: string) => void;
    cacheClear: () => void;
    licenseKey: string;
  }
}

const globalCache = new CacheableMemory();

export const contextCache = (): MiddlewareHandler => {
  return async (c, next) => {
    c.set("cache", globalCache);

    c.set("cacheSet", <T>(key: string, value: T, ttl?: string | number) => {
      globalCache.set(key, value, ttl);
    });

    c.set("cacheGet", <T>(key: string): T | undefined => {
      return globalCache.get<T>(key);
    });

    c.set("cacheHas", (key: string): boolean => {
      return globalCache.has(key);
    });

    c.set("cacheDelete", (key: string): void => {
      globalCache.delete(key);
    });

    c.set("cacheClear", (): void => {
      globalCache.clear();
    });

    await next();
  };
};
