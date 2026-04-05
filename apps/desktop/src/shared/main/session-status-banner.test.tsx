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

import type { BottomAccessoryState } from "~/session/components/bottom-accessory";

function BannerPublisher({
  skipReason,
  bottomAccessoryState = null,
}: {
  skipReason: string | null;
  bottomAccessoryState?: BottomAccessoryState;
}) {
  useSessionStatusBanner({
    skipReason,
    bottomAccessoryState,
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
        <BannerPublisher
          skipReason={null}
          bottomAccessoryState={{ mode: "playback", expanded: false }}
        />
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
          bottomAccessoryState={{ mode: "live", expanded: false }}
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

  it("positions skip reasons above expanded post-session transcript", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher
          skipReason="Microphone access is disabled"
          bottomAccessoryState={{ mode: "playback", expanded: true }}
        />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banners = screen.getAllByText("Microphone access is disabled");
    const banner = banners[banners.length - 1];
    expect(banner).toBeTruthy();
    expect(banner?.className).toContain("bottom-[224px]");
  });
});
