import { expect, it } from "vitest";

import { selectRecoveredWords } from "./recovered-transcript";

const word = (id: string, start: number, end: number, text = id) => ({
  id,
  text,
  start_ms: start,
  end_ms: end,
  channel: 0,
});

it("fills the gap without replacing edited text or duplicating overlap", () => {
  const existing = [word("saved", 0, 1000, "User correction")];
  const recovered = [
    word("overlap", 800, 1200),
    word("missing", 1500, 1900),
    word("outside", 3000, 3500),
  ];
  expect(
    selectRecoveredWords(recovered, existing, [{ start: 900, end: 2500 }]),
  ).toEqual([recovered[1]]);
});

it("is idempotent when a committed repair is retried before audio acknowledgement", () => {
  const saved = word("repaired", 1000, 1500);
  const retry = word("different-provider-id", 1020, 1520);
  expect(
    selectRecoveredWords([retry], [saved], [{ start: 0, end: 2000 }]),
  ).toEqual([]);
});
