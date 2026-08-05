import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SessionAttachmentControls } from "./attachment-controls";
import type { SessionShareAttachment } from "./attachments";

const audio: SessionShareAttachment = {
  id: "audio-1",
  filename: "audio.mp3",
  contentType: "audio/mpeg",
  sizeBytes: 42,
  sha256: "a".repeat(64),
  sourceType: "session_audio",
  sourceId: "session-1",
  cloudSyncEnabled: false,
  cloudObjectKey: "",
  localAvailability: "present",
  transferDirection: null,
  transferPhase: null,
  transferError: "",
};

afterEach(cleanup);

describe("SessionAttachmentControls", () => {
  it("stays hidden when the session has no audio recording", () => {
    const attachment = {
      ...audio,
      id: "attachment-1",
      sourceType: "note_upload",
    };

    render(
      <SessionAttachmentControls
        attachments={[attachment]}
        sharedAttachmentIds={new Map()}
        canShare
        pendingAttachmentId={null}
        onShareChange={vi.fn()}
      />,
    );

    expect(screen.queryByRole("switch", { name: "Share audio" })).toBeNull();
  });

  it("shares and unshares the session audio with one switch", () => {
    const onShareChange = vi.fn();
    const view = render(
      <SessionAttachmentControls
        attachments={[audio]}
        sharedAttachmentIds={new Map()}
        canShare
        pendingAttachmentId={null}
        onShareChange={onShareChange}
      />,
    );

    fireEvent.click(screen.getByRole("switch", { name: "Share audio" }));
    expect(onShareChange).toHaveBeenCalledWith(audio, true);

    view.rerender(
      <SessionAttachmentControls
        attachments={[audio]}
        sharedAttachmentIds={new Map([[audio.id, "shared-audio-1"]])}
        canShare
        pendingAttachmentId={null}
        onShareChange={onShareChange}
      />,
    );

    fireEvent.click(screen.getByRole("switch", { name: "Share audio" }));
    expect(onShareChange).toHaveBeenLastCalledWith(audio, false);
  });
});
