import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: "macos",
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.platform,
}));

import { TrialEndedDialog } from "./trial-ended-dialog";

describe("TrialEndedDialog", () => {
  afterEach(cleanup);

  it("keeps the local transcription message on macOS", () => {
    mocks.platform = "macos";

    render(
      <TrialEndedDialog open onOpenChange={() => {}} onUpgrade={() => {}} />,
    );

    expect(
      screen.getByText(/Free local transcription still works/),
    ).toBeTruthy();
  });

  it.each(["windows", "linux"])(
    "does not promise local transcription on %s",
    (platform) => {
      mocks.platform = platform;

      render(
        <TrialEndedDialog open onOpenChange={() => {}} onUpgrade={() => {}} />,
      );

      expect(screen.queryByText(/Free local transcription/)).toBeNull();
      expect(
        screen.getByText(/configure your own transcription provider/),
      ).toBeTruthy();
    },
  );
});
