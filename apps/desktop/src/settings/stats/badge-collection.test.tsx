import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { BadgeId } from "./badges";

const mocks = vi.hoisted(() => ({
  collection: {
    data: {} as Partial<Record<BadgeId, string>>,
    isLoading: false,
    error: null as Error | null,
  },
  collect: vi.fn(),
  onboarding: vi.fn(),
}));
vi.mock("./badge-queries", () => ({
  useCollectedBadges: () => mocks.collection,
  collectBadges: mocks.collect,
}));
vi.mock("~/types/tauri.gen", () => ({
  commands: { getOnboardingNeeded: mocks.onboarding },
}));

import { BadgeCollection, BadgeGallery } from "./badge-collection";
import { getBadgeProgress } from "./badges";

const now = new Date("2026-09-08T12:00:00Z");
const record = {
  session_id: "meeting",
  started_at_ms: Date.parse("2026-09-01T12:00:00Z"),
  created_at: "2026-09-01T12:00:00Z",
  duration_ms: 60_000,
};

describe("badge collection", () => {
  afterEach(cleanup);
  beforeEach(() => {
    mocks.collection = { data: {}, isLoading: false, error: null };
    mocks.collect.mockReset().mockResolvedValue(undefined);
    mocks.onboarding
      .mockReset()
      .mockResolvedValue({ status: "ok", data: false });
  });

  it("collects newly eligible badges once and waits for confirmation before marking them collected", async () => {
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    const view = () => (
      <QueryClientProvider client={client}>
        <BadgeCollection
          records={[record]}
          now={now}
          timezone="UTC"
          weekStartsOn={1}
        />
      </QueryClientProvider>
    );
    const { rerender } = render(view());
    await waitFor(() =>
      expect(mocks.collect).toHaveBeenCalledWith(
        "00000000-0000-0000-0000-000000000000",
        ["hello", "all-set", "first-words"],
      ),
    );
    expect(screen.getByText("0 of 9 collected")).toBeTruthy();
    mocks.collection.data = {
      hello: now.toISOString(),
      "all-set": now.toISOString(),
      "first-words": now.toISOString(),
    };
    rerender(view());
    expect(screen.getByText("3 of 9 collected")).toBeTruthy();
    expect(mocks.collect).toHaveBeenCalledTimes(1);
  });

  it("offers a retry when saving a badge fails", async () => {
    mocks.collect.mockRejectedValueOnce(new Error("Disk unavailable"));
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    render(
      <QueryClientProvider client={client}>
        <BadgeCollection records={[]} now={now} weekStartsOn={1} />
      </QueryClientProvider>,
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Couldn't save",
    );
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(mocks.collect).toHaveBeenCalledTimes(2));
  });

  it("keeps earned badges after history shrinks and opens their collection details", () => {
    render(
      <BadgeGallery
        progress={getBadgeProgress({
          records: [],
          now,
          weekStartsOn: 1,
          signedUp: false,
          onboardingComplete: false,
        })}
        collected={{ "first-words": "2026-09-01T12:00:00Z" }}
      />,
    );
    expect(screen.getByText("1 of 9 collected")).toBeTruthy();
    const first = screen.getByRole("button", { name: "First Words" });
    expect(within(first).getByText("Collected")).toBeTruthy();
    fireEvent.click(first);
    const dialog = screen.getByRole("dialog", { name: "First Words" });
    expect(within(dialog).getByText(/Collected Sep 1, 2026/)).toBeTruthy();
    expect(within(dialog).queryByRole("progressbar")).toBeNull();
  });

  it("shows a clear requirement and progress for a future badge", () => {
    render(
      <BadgeGallery
        progress={getBadgeProgress({
          records: [record],
          now,
          weekStartsOn: 1,
          signedUp: false,
          onboardingComplete: false,
        })}
        collected={{}}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Good Listener" }));
    const dialog = screen.getByRole("dialog", { name: "Good Listener" });
    expect(within(dialog).getByText("1 / 10 conversations")).toBeTruthy();
    expect(
      within(dialog).getByRole("progressbar").getAttribute("aria-valuenow"),
    ).toBe("1");
  });

  it("does not award badges if the saved collection cannot be loaded", async () => {
    mocks.collection.error = new Error("Database unavailable");
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <BadgeCollection records={[record]} now={now} weekStartsOn={1} />
      </QueryClientProvider>,
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Couldn't load",
    );
    expect(mocks.collect).not.toHaveBeenCalled();
  });

  it("does not mistake an onboarding read failure for completed onboarding", async () => {
    mocks.onboarding.mockResolvedValue({
      status: "error",
      error: "Store unavailable",
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <BadgeCollection records={[]} now={now} weekStartsOn={1} />
      </QueryClientProvider>,
    );
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Couldn't load",
    );
    expect(mocks.collect).not.toHaveBeenCalled();
  });
});
