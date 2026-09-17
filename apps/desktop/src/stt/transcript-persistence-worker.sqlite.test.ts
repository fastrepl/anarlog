import { createRequire } from "node:module";
import { expect, it, vi } from "vitest";

import { createTranscriptPersistenceWorker } from "./transcript-persistence-worker";

const { DatabaseSync } = createRequire(import.meta.url)(
  "node:sqlite",
) as typeof import("node:sqlite");

it("retries the failed transcript after SQLite runs out of pages and recovers", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec(
    "PRAGMA page_size = 512; CREATE TABLE words (id TEXT PRIMARY KEY, text TEXT)",
  );
  const pages = Number(db.prepare("PRAGMA page_count").get()!.page_count);
  db.exec(`PRAGMA max_page_count = ${pages}`);
  const onError = vi.fn();
  const worker = createTranscriptPersistenceWorker(
    async (delta) => {
      for (const word of delta.new_words) {
        db.prepare("INSERT OR REPLACE INTO words VALUES (?, ?)").run(
          word.id,
          word.text,
        );
      }
    },
    onError,
    { batchWindowMs: 0 },
  );
  try {
    worker.enqueue({
      new_words: [
        {
          id: "lost-before-fix",
          text: "speech ".repeat(1000),
          start_ms: 0,
          end_ms: 1000,
          channel: 0,
          state: "final",
          speaker_index: null,
        },
      ],
      replaced_ids: [],
      partials: [],
    });
    await worker.flush();
    expect(onError.mock.calls[0]?.[0]).toMatchObject({ errcode: 13 });
    expect(db.prepare("SELECT count(*) AS count FROM words").get()!.count).toBe(
      0,
    );
    db.exec("PRAGMA max_page_count = 1000");
    await worker.flush();
    expect(worker.hasPendingFailure()).toBe(false);
    expect(db.prepare("SELECT id FROM words").get()!.id).toBe(
      "lost-before-fix",
    );
  } finally {
    worker.dispose();
    db.close();
  }
});
