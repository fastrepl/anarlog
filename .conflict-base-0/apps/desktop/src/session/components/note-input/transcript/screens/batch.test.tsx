import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BatchState } from "./batch";

vi.mock("@hypr/ui/components/ui/dancing-sticks", () => ({
  DancingSticks: () => null,
}));

vi.mock("~/stt/contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({ live: { amplitude: { mic: 0, speaker: 0 } } }),
}));

describe("BatchState", () => {
  afterEach(cleanup);

  it("identifies intentional batch transcription", () => {
    render(<BatchState requestedLiveTranscription={false} error={null} />);

    expect(screen.getByText("Batch transcription mode")).not.toBeNull();
    expect(
      screen.getByText(
        "Recording continues. Your transcript will be generated after you stop.",
      ),
    ).not.toBeNull();
  });

  it("explains transient fallback and reconnection", () => {
    render(
      <BatchState
        requestedLiveTranscription
        error={{ type: "connection_timeout" }}
      />,
    );

    expect(screen.getByText("Reconnecting live transcription")).not.toBeNull();
    expect(screen.getByText(/while we reconnect/)).not.toBeNull();
    expect(screen.getByText(/complete transcript/)).not.toBeNull();
  });

  it("does not promise reconnection after an authentication failure", () => {
    render(
      <BatchState
        requestedLiveTranscription
        error={{ type: "authentication_failed", provider: "Deepgram" }}
      />,
    );

    expect(screen.getByText("Live transcription stopped")).not.toBeNull();
    expect(screen.queryByText(/while we reconnect/)).toBeNull();
    expect(screen.getByText(/complete transcript/)).not.toBeNull();
  });
});
