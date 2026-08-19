import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { type ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  TranscriptSelectButton,
  TranscriptSelectToolbar,
} from "./select-toolbar";

afterEach(cleanup);

vi.mock("@anlg/ui/components/ui/popover", () => ({
  Popover: ({ children }: { children: ReactNode }) => <>{children}</>,
  PopoverTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  PopoverContent: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("./speaker-assign", () => ({
  SpeakerParticipantPicker: ({
    onSelect,
  }: {
    onSelect: (humanId: string) => void;
  }) => (
    <button type="button" onClick={() => void onSelect("human-1")}>
      Confirm
    </button>
  ),
}));

describe("TranscriptSelectToolbar", () => {
  it("keeps bulk reassignment disabled until entries are selected", () => {
    render(
      <TranscriptSelectToolbar
        selection={null}
        entryCount={0}
        onSelectAll={vi.fn()}
        onClear={vi.fn()}
        onDone={vi.fn()}
        onAssignSpeaker={vi.fn()}
      />,
    );

    expect(
      (
        screen.getByRole("button", {
          name: "Change speaker",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "Clear" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("assigns the current selection and then clears it", async () => {
    const onAssignSpeaker = vi.fn(() => Promise.resolve());
    const onClear = vi.fn();
    const selection = {
      sessionId: "session-1",
      text: "Hello",
      startMs: 0,
      groups: [],
    };

    render(
      <TranscriptSelectToolbar
        selection={selection}
        entryCount={2}
        onSelectAll={vi.fn()}
        onClear={onClear}
        onDone={vi.fn()}
        onAssignSpeaker={onAssignSpeaker}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));

    await waitFor(() => {
      expect(onAssignSpeaker).toHaveBeenCalledWith(selection, "human-1");
      expect(onClear).toHaveBeenCalled();
    });
  });
});

describe("TranscriptSelectButton", () => {
  it("toggles select mode", () => {
    const onSelectModeChange = vi.fn();
    render(
      <TranscriptSelectButton
        selectMode={false}
        onSelectModeChange={onSelectModeChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    expect(onSelectModeChange).toHaveBeenCalledWith(true);
  });
});
