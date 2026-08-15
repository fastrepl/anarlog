import { liveQueryClient } from "~/db";

const SEARCH_INDEX_FINALIZATION_TIMEOUT_MS = 10_000;
const SEARCH_INDEX_POLL_INTERVAL_MS = 50;

type SearchIndexGenerationRow = {
  generation: number;
  acknowledged_generation: number;
};

export async function waitForSessionSearchIndex(
  sessionId: string,
  options: {
    timeoutMs?: number;
    pollIntervalMs?: number;
  } = {},
): Promise<void> {
  const timeoutMs = options.timeoutMs ?? SEARCH_INDEX_FINALIZATION_TIMEOUT_MS;
  const pollIntervalMs =
    options.pollIntervalMs ?? SEARCH_INDEX_POLL_INTERVAL_MS;
  const deadline = Date.now() + timeoutMs;
  let targetGeneration: number | undefined;
  let lastError: unknown;

  while (Date.now() <= deadline) {
    try {
      const rows = await liveQueryClient.execute<SearchIndexGenerationRow>(
        `
          SELECT generation, acknowledged_generation
          FROM search_index_dirty
          WHERE entity_type = 'session' AND entity_id = ?
          LIMIT 1
        `,
        [sessionId],
      );
      const row = rows[0];
      if (!row) return;

      targetGeneration ??= Number(row.generation);
      if (Number(row.acknowledged_generation) >= targetGeneration) return;
      lastError = undefined;
    } catch (error) {
      lastError = error;
    }

    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }

  const suffix = lastError instanceof Error ? `: ${lastError.message}` : "";
  throw new Error(
    `Search index did not acknowledge session ${sessionId} within ${timeoutMs}ms${suffix}`,
  );
}
