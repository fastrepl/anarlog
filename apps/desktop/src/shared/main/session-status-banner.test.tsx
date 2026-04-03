import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

let hasUndoDeleteToast = false;

vi.mock("./content-offset", () => ({
  useMainContentCenterOffset: () => 24,
}));

vi.mock("~/store/zustand/undo-delete", () => ({
  useUndoDelete: (
    selector: (state: { pendingDeletions: Record<string, unknown> }) => unknown,
  ) =>
    selector({
      pendingDeletions: hasUndoDeleteToast ? { "session-1": {} } : {},
    }),
}));

import {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";

function BannerPublisher({
  skipReason,
  bottomAccessoryKind = null,
}: {
  skipReason: string | null;
  bottomAccessoryKind?:
    | "live_transcript"
    | "live_transcript_expanded"
    | "playback"
    | null;
}) {
  useSessionStatusBanner({
    skipReason,
    bottomAccessoryKind,
  });
  return null;
}

describe("MainSessionStatusBannerHost", () => {
  beforeEach(() => {
    hasUndoDeleteToast = false;
  });

  it("does not render without a skip reason", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher skipReason={null} bottomAccessoryKind="playback" />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    expect(screen.queryByText("Ask for consent when using Char")).toBeNull();
  });

  it("prefers the skip reason and stacks above the undo-delete toast", () => {
    hasUndoDeleteToast = true;

    render(
      <SessionStatusBannerProvider>
        <BannerPublisher skipReason="Microphone access is disabled" />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banner = screen.getByText("Microphone access is disabled");
    expect(banner.className).toContain("bottom-1");
    expect(banner.className).toContain("text-red-400");
  });

  it("positions skip reasons above the bottom accessory", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher
          skipReason="Microphone access is disabled"
          bottomAccessoryKind="live_transcript"
        />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banners = screen.getAllByText("Microphone access is disabled");
    const banner = banners[banners.length - 1];
    expect(banner).toBeTruthy();
    expect(banner?.className).toContain("bottom-[76px]");
    expect(banner?.getAttribute("style")).toContain("calc(50% + 24px)");
  });
});
