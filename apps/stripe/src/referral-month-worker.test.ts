import { expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

test("a replayed trial webhook and failed reward persistence cannot extend a trial twice", async () => {
  // Isolate provider module mocks from other billing tests.
  const child = Bun.spawn(
    [
      process.execPath,
      fileURLToPath(
        new URL("./referral-month-worker.fixture.ts", import.meta.url),
      ),
    ],
    {
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  try {
    const [exitCode, stderr] = await Promise.all([
      child.exited,
      new Response(child.stderr).text(),
    ]);
    expect(stderr).toBe("");
    expect(exitCode).toBe(0);
  } finally {
    child.kill();
  }
}, 30_000);
