const TRANSIENT_DATABASE_CODES = new Set([
  "08000",
  "08001",
  "08003",
  "08006",
  "40001",
  "40P01",
  "53300",
  "57P01",
  "57P02",
  "57P03",
  "PGRST000",
  "PGRST001",
  "PGRST002",
  "PGRST003",
  "ECONNREFUSED",
  "ECONNRESET",
  "ETIMEDOUT",
]);

export async function withDatabaseRetry<T>(operation: () => Promise<T>) {
  const startedAt = performance.now();
  const delays = [500, 1_000, 2_000];

  for (let attempt = 0; ; attempt++) {
    try {
      return await operation();
    } catch (error) {
      const delay = delays[attempt];
      // Only replay idempotent database work, and don't compound slow failures.
      if (
        delay === undefined ||
        performance.now() - startedAt + delay >= 10_000 ||
        !isTransientDatabaseError(error)
      ) {
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }
}

function isTransientDatabaseError(error: unknown) {
  if (!error || typeof error !== "object") return false;

  if (
    "code" in error &&
    typeof error.code === "string" &&
    TRANSIENT_DATABASE_CODES.has(error.code)
  ) {
    return true;
  }

  return (
    "status" in error &&
    (error.status === 502 || error.status === 503 || error.status === 504)
  );
}
