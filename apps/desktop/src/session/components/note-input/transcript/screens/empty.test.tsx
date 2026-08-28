import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TranscriptEmptyState } from "./empty";

describe("TranscriptEmptyState", () => {
  afterEach(() => {
    cleanup();
  });

  it("lets users stop batch transcription", () => {
    const onStopTranscription = vi.fn();

    render(
      <TranscriptEmptyState
        isBatching
        phase="transcribing"
        onStopTranscription={onStopTranscription}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop transcription" }));

    expect(onStopTranscription).toHaveBeenCalledTimes(1);
  });

  it("hides the stop control while importing audio", () => {
    render(<TranscriptEmptyState isBatching phase="importing" />);

    expect(
      screen.queryByRole("button", { name: "Stop transcription" }),
    ).toBeNull();
  });

  it("uses the same error hierarchy and offers re-transcription", () => {
    const onRetranscribe = vi.fn();

    render(
      <TranscriptEmptyState
        error="The transcription provider timed out."
        onRetranscribe={onRetranscribe}
      />,
    );

    expect(screen.getByRole("alert")).not.toBeNull();
    expect(screen.getByText("Transcription failed").className).toContain(
      "text-base",
    );
    expect(
      screen.getByText("The transcription provider timed out.").className,
    ).toContain("text-sm");

    fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));
    expect(onRetranscribe).toHaveBeenCalledTimes(1);
  });

  it("offers re-transcription instead of replacing existing audio", () => {
    const onRetranscribe = vi.fn();

    render(
      <TranscriptEmptyState
        hasAudio
        onRetranscribe={onRetranscribe}
        onUploadAudio={vi.fn()}
        onUploadTranscript={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));

    expect(onRetranscribe).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Upload audio" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Upload transcript" }),
    ).not.toBeNull();
    expect(screen.getByText("Audio available")).not.toBeNull();
    expect(screen.getByText(/Re-transcribe this audio/)).not.toBeNull();
    expect(screen.queryByText(/refresh button/i)).toBeNull();
  });
});
