import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import { RecordingImportWorker } from "./recording-import";

const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  transaction: vi.fn(),
  transcribe: vi.fn(),
  summarize: vi.fn(),
  billingReady: true,
  authSession: {} as object | null,
}));
vi.mock("@anlg/plugin-settings", () => ({
  commands: { vaultBase: async () => ({ status: "ok", data: "/vault" }) },
}));
vi.mock("~/settings/queries", () => ({ useSettingsReady: () => true }));
vi.mock("~/settings/providers", () => ({
  useAiProvidersState: () => ({ isReady: true }),
}));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isReady: mocks.billingReady }),
}));
vi.mock("~/auth", () => ({
  useAuth: () => ({ session: mocks.authSession }),
}));
vi.mock("~/db", () => ({
  liveQueryClient: { execute: mocks.execute },
  executeTransaction: mocks.transaction,
}));
vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: { audioImport: async () => ({ status: "ok", data: "/audio.wav" }) },
}));
vi.mock("@anlg/plugin-fs2", () => ({
  commands: { remove: async () => ({ status: "ok" }) },
}));
vi.mock("~/db/cloudsync-activity", () => ({
  withCloudsyncActivity: (_kind: string, _id: string, run: () => unknown) =>
    run(),
}));
vi.mock("~/session/attachments", () => ({
  catalogLocalSessionAudio: async () => {},
}));
vi.mock("~/session/queries", () => ({ useSession: () => ({ id: "meeting" }) }));
vi.mock("~/stt/useRunBatch", () => ({ useRunBatch: () => mocks.transcribe }));
vi.mock("~/services/enhancer", () => ({
  getEnhancerService: () => ({ enhanceAndWait: mocks.summarize }),
}));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

function setupJob(fails: boolean) {
  let status = "queued";
  let summaryStatus = "pending";
  let error: string | null = null;
  let finish: (() => void) | undefined;
  const summary = new Promise<void>((resolve) => {
    finish = resolve;
  });
  mocks.transcribe.mockResolvedValue(undefined);
  mocks.summarize.mockImplementation(async () => {
    await summary;
    if (fails) throw new Error("provider-secret-must-not-leak");
    return "done";
  });
  mocks.execute.mockImplementation(async (sql: string) => {
    if (sql.includes("RETURNING id")) {
      if (status !== "queued") return [];
      status = "importing";
      return [{ id: "job" }];
    }
    if (sql.includes("file_path != ''")) return [];
    return ["done", "failed"].includes(status)
      ? []
      : [{ id: "job", meeting_id: "meeting", file_path: "/staged.wav" }];
  });
  mocks.transaction.mockImplementation(
    async (statements: { params: unknown[] }[]) => {
      for (const statement of statements) {
        if (statement.params.length === 5) {
          [status, summaryStatus, error] = statement.params as [
            string,
            string,
            string | null,
            string,
            string,
          ];
        }
      }
    },
  );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const view = render(
    <StrictMode>
      <QueryClientProvider client={client}>
        <RecordingImportWorker />
      </QueryClientProvider>
    </StrictMode>,
  );
  return {
    client,
    view,
    finishSummary: () => finish?.(),
    state: () => ({ status, summaryStatus, error }),
  };
}

it.each([false, true])(
  "survives StrictMode and waits for the summary outcome (failure: %s)",
  async (fails) => {
    vi.useFakeTimers();
    mocks.billingReady = true;
    mocks.authSession = {};
    const job = setupJob(fails);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(job.state().status).toBe("summarizing");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(12000);
    });
    expect(job.state().status).toBe("summarizing");
    await act(async () => {
      job.finishSummary();
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(job.state().status).toBe(fails ? "failed" : "done");
    expect(job.state().summaryStatus).toBe(fails ? "failed" : "done");
    expect(job.state().error ?? "").not.toContain("provider-secret");
    job.client.clear();
  },
);

it("runs signed-out imports and finishes them after the worker unmounts", async () => {
  vi.useFakeTimers();
  mocks.billingReady = false;
  mocks.authSession = null;
  const job = setupJob(false);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(100);
  });
  expect(job.state().status).toBe("summarizing");
  job.view.unmount();
  await act(async () => {
    job.finishSummary();
    await vi.advanceTimersByTimeAsync(100);
  });
  expect(job.state().status).toBe("done");
  job.client.clear();
});
