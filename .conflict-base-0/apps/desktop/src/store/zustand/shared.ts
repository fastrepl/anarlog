export function wrapSliceWithLogging<T extends Record<string, unknown>>(
  name: string,
  slice: T,
): T {
  if (process.env.NODE_ENV === "production") {
    return slice;
  }

  const wrapped: Record<string, unknown> = {};

  for (const key in slice) {
    const value = slice[key];
    if (typeof value === "function") {
      wrapped[key] = (...args: unknown[]) => {
        console.log(`[${name}] ${key}`, args);
        return value(...args);
      };
    } else {
      wrapped[key] = value;
    }
  }

  return wrapped as T;
}
