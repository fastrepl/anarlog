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
  showConsentBanner,
  showTimeline,
}: {
  skipReason: string | null;
  showConsentBanner: boolean;
  showTimeline: boolean;
}) {
  useSessionStatusBanner({
    skipReason,
    showConsentBanner,
    showTimeline,
  });
  return null;
}

describe("MainSessionStatusBannerHost", () => {
  beforeEach(() => {
    hasUndoDeleteToast = false;
  });

  it("renders the consent banner using shell-managed positioning", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher
          skipReason={null}
          showConsentBanner={true}
          showTimeline={true}
        />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banner = screen.getByText("Ask for consent when using Char");
    expect(banner.className).toContain("bottom-[76px]");
    expect(banner.getAttribute("style")).toContain("calc(50% + 24px)");
  });

  it("prefers the skip reason and stacks above the undo-delete toast", () => {
    hasUndoDeleteToast = true;

    render(
      <SessionStatusBannerProvider>
        <BannerPublisher
          skipReason="Microphone access is disabled"
          showConsentBanner={true}
          showTimeline={false}
        />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banner = screen.getByText("Microphone access is disabled");
    expect(banner.className).toContain("bottom-1");
    expect(banner.className).toContain("text-red-400");
  });
});
