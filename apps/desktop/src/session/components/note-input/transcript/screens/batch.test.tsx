import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BatchState } from "./batch";

vi.mock("@anlg/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => null,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      live: {
        amplitude: { mic: 0, speaker: 0 },
        degraded: { type: "stream_error", message: "deepgram: overloaded" },
      },
    }),
}));

vi.mock("~/settings/queries", () => ({
  useStoredSettingValue: () => ({ value: "deepgram", hasValue: true }),
}));

describe("BatchState", () => {
  afterEach(cleanup);

  it("identifies intentional batch transcription", () => {
    render(<BatchState requestedLiveTranscription={false} />);

    expect(screen.getByText("Batch transcription mode")).not.toBeNull();
    expect(
      screen.getByText(
        "Recording continues. Your transcript will be generated after you stop.",
      ),
    ).not.toBeNull();
  });

  it("reassures that audio is saved when live transcription fails", () => {
    render(<BatchState requestedLiveTranscription />);

    expect(screen.getByRole("status").textContent).toMatch(
      /^Live transcript paused because Deepgram is having an outage\./,
    );
  });
});
