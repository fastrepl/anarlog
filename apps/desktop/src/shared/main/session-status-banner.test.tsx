import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./content-offset", () => ({
  useMainContentCenterOffset: () => 24,
}));

import {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";

function BannerPublisher({ skipReason }: { skipReason: string | null }) {
  useSessionStatusBanner({
    skipReason,
  });
  return null;
}

describe("MainSessionStatusBannerHost", () => {
  it("does not render without a skip reason", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher skipReason={null} />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    expect(screen.queryByText("Ask for consent when using Anarlog")).toBeNull();
  });

  it("positions skip reasons at the bottom of the main surface", () => {
    render(
      <SessionStatusBannerProvider>
        <BannerPublisher skipReason="Microphone access is disabled" />
        <MainSessionStatusBannerHost />
      </SessionStatusBannerProvider>,
    );

    const banners = screen.getAllByText("Microphone access is disabled");
    const banner = banners[banners.length - 1];
    expect(banner).toBeTruthy();
    expect(banner?.className).toContain("bottom-6");
    expect(banner?.getAttribute("style")).toContain("calc(50% + 24px)");
  });
});
