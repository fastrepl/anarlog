import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  createElement,
  type MouseEvent,
  type ReactElement,
  type ReactNode,
} from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  buildEventSpeakerParticipantOptions,
  buildCreateSpeakerParticipantOption,
  buildSpeakerParticipantGroups,
  getAssignmentAnchorWordId,
  getAssignmentWordIds,
  SpeakerAssignPopover,
  type SpeakerParticipantOption,
} from "./speaker-assign";

import type { Segment } from "~/stt/live-segment";

const {
  useCellMock,
  useQueriesMock,
  useRowIdsMock,
  useSliceRowIdsMock,
  useStoreMock,
  useTableMock,
  useValueMock,
} = vi.hoisted(() => ({
  useCellMock: vi.fn(),
  useQueriesMock: vi.fn(),
  useRowIdsMock: vi.fn(),
  useSliceRowIdsMock: vi.fn(),
  useStoreMock: vi.fn(),
  useTableMock: vi.fn(),
  useValueMock: vi.fn(),
}));

vi.mock("@hypr/ui/components/ui/popover", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  const PopoverContext = React.createContext<{
    open: boolean;
    onOpenChange: (open: boolean) => void;
  } | null>(null);

  return {
    AppFloatingPanel: ({
      children,
      className,
    }: {
      children: ReactNode;
      className?: string;
    }) => React.createElement("div", { className }, children),
    Popover: ({
      children,
      open,
      onOpenChange,
    }: {
      children: ReactNode;
      open: boolean;
      onOpenChange: (open: boolean) => void;
    }) =>
      React.createElement(
        PopoverContext.Provider,
        { value: { open, onOpenChange } },
        children,
      ),
    PopoverContent: ({
      children,
      className,
    }: {
      children: ReactNode;
      className?: string;
    }) => {
      const context = React.useContext(PopoverContext);
      return context?.open
        ? React.createElement(
            "div",
            { "data-popover-content": true, className },
            children,
          )
        : null;
    },
    PopoverTrigger: ({
      children,
    }: {
      children: ReactElement<{
        onClick?: (event: MouseEvent) => void;
      }>;
    }) => {
      const context = React.useContext(PopoverContext);
      return React.cloneElement(children, {
        onClick: (event: MouseEvent) => {
          children.props.onClick?.(event);
          context?.onOpenChange(true);
        },
      });
    },
  };
});

vi.mock("@hypr/ui/components/ui/checkbox", () => ({
  Checkbox: ({
    checked,
    onCheckedChange,
  }: {
    checked: boolean;
    onCheckedChange: (checked: boolean) => void;
  }) =>
    createElement("input", {
      type: "checkbox",
      checked,
      onChange: (event) => onCheckedChange(event.currentTarget.checked),
    }),
}));

vi.mock("~/store/tinybase/store/main", () => ({
  STORE_ID: "main",
  INDEXES: {
    sessionParticipantsBySession: "sessionParticipantsBySession",
  },
  QUERIES: {
    sessionParticipantsWithDetails: "sessionParticipantsWithDetails",
  },
  UI: {
    useCell: useCellMock,
    useQueries: useQueriesMock,
    useRowIds: useRowIdsMock,
    useSliceRowIds: useSliceRowIdsMock,
    useStore: useStoreMock,
    useTable: useTableMock,
    useValue: useValueMock,
  },
}));

beforeEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function option(
  id: string,
  name: string,
  overrides: Partial<SpeakerParticipantOption> = {},
): SpeakerParticipantOption {
  return {
    id,
    name,
    isSessionParticipant: false,
    ...overrides,
  };
}

describe("SpeakerAssignPopover", () => {
  it("assigns only after confirmation and defaults to all matching segments", () => {
    const cells = new Map([
      [
        "words",
        JSON.stringify([
          {
            id: "word-1",
            text: "hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
        ]),
      ],
      ["speaker_hints", JSON.stringify([])],
    ]);
    const store = {
      getCell: vi.fn((_tableId: string, _rowId: string, cellId: string) =>
        cells.get(cellId),
      ),
      setCell: vi.fn(
        (_tableId: string, _rowId: string, cellId: string, value: string) => {
          cells.set(cellId, value);
        },
      ),
      getRow: vi.fn(() => ({
        name: "Alice",
        email: "",
      })),
      setRow: vi.fn(),
    };

    useCellMock.mockReturnValue("session-1");
    useQueriesMock.mockReturnValue({ getResultRow: vi.fn() });
    useRowIdsMock.mockReturnValue(["human-1"]);
    useSliceRowIdsMock.mockReturnValue([]);
    useStoreMock.mockReturnValue(store);
    useTableMock.mockReturnValue({});
    useValueMock.mockReturnValue("user-1");

    render(
      createElement(SpeakerAssignPopover, {
        segment: {
          id: "segment-1",
          key: {
            channel: "RemoteParty",
            speaker_index: 2,
            speaker_human_id: null,
          },
          start_ms: 0,
          end_ms: 100,
          text: "hello",
          words: [
            {
              id: "word-1",
              text: "hello",
              start_ms: 0,
              end_ms: 100,
              channel: "RemoteParty",
              is_final: true,
            },
          ],
        } as Segment,
        transcriptId: "transcript-1",
        color: "red",
        label: "Speaker 2",
      }),
    );

    const trigger = screen.getByRole("button", { name: "Speaker 2" });
    expect(trigger.className).toContain("rounded-full");
    expect(trigger.className).toContain("pr-2");
    expect(trigger.className).toContain("hover:underline");
    expect(trigger.className).toContain("focus-visible:underline");
    expect(trigger.className).not.toContain("hover:bg-accent");
    expect(trigger.className.split(/\s+/)).not.toContain("underline");
    expect(trigger.className).not.toContain("px-2");
    expect(trigger.className).not.toContain("-ml-2");

    fireEvent.click(trigger);
    expect(trigger.className.split(/\s+/)).toContain("underline");
    fireEvent.click(screen.getByRole("button", { name: "Alice" }));
    expect(cells.get("speaker_hints")).toBe(JSON.stringify([]));

    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));

    expect(cells.get("speaker_hints")).toBe(
      JSON.stringify([
        {
          id: "word-1:user_speaker_assignment",
          word_id: "word-1",
          type: "user_speaker_assignment",
          value: JSON.stringify({ human_id: "human-1" }),
        },
      ]),
    );
  });

  it("uses segment scope when the matching-segments checkbox is off", () => {
    const cells = new Map([
      [
        "words",
        JSON.stringify([
          {
            id: "word-1",
            text: "hello",
            start_ms: 0,
            end_ms: 100,
            channel: 1,
          },
        ]),
      ],
      ["speaker_hints", JSON.stringify([])],
    ]);
    const store = {
      getCell: vi.fn((_tableId: string, _rowId: string, cellId: string) =>
        cells.get(cellId),
      ),
      setCell: vi.fn(
        (_tableId: string, _rowId: string, cellId: string, value: string) => {
          cells.set(cellId, value);
        },
      ),
      getRow: vi.fn(() => ({
        name: "Alice",
        email: "",
      })),
      setRow: vi.fn(),
    };

    useCellMock.mockReturnValue("session-1");
    useQueriesMock.mockReturnValue({ getResultRow: vi.fn() });
    useRowIdsMock.mockReturnValue(["human-1"]);
    useSliceRowIdsMock.mockReturnValue([]);
    useStoreMock.mockReturnValue(store);
    useTableMock.mockReturnValue({});
    useValueMock.mockReturnValue("user-1");

    render(
      createElement(SpeakerAssignPopover, {
        segment: {
          id: "segment-1",
          key: {
            channel: "RemoteParty",
            speaker_index: 2,
            speaker_human_id: null,
          },
          start_ms: 0,
          end_ms: 100,
          text: "hello",
          words: [
            {
              id: "word-1",
              text: "hello",
              start_ms: 0,
              end_ms: 100,
              channel: "RemoteParty",
              is_final: true,
            },
          ],
        } as Segment,
        transcriptId: "transcript-1",
        color: "red",
        label: "Speaker 2",
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Speaker 2" }));
    fireEvent.click(screen.getByRole("checkbox"));
    fireEvent.click(screen.getByRole("button", { name: "Alice" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));

    expect(cells.get("speaker_hints")).toBe(
      JSON.stringify([
        {
          id: "word-1:user_speaker_assignment:segment",
          word_id: "word-1",
          type: "user_speaker_assignment",
          value: JSON.stringify({
            human_id: "human-1",
            scope: "segment",
            word_ids: ["word-1"],
          }),
        },
      ]),
    );
  });
});

describe("buildSpeakerParticipantGroups", () => {
  it("falls back to contacts when a transcript has no session participants", () => {
    const groups = buildSpeakerParticipantGroups({
      sessionParticipants: [],
      contacts: [option("human-1", "Alice")],
      query: "",
    });

    expect(groups).toEqual([
      {
        title: "People",
        options: [option("human-1", "Alice")],
      },
    ]);
  });

  it("keeps participants first and excludes duplicate people", () => {
    const participant = option("human-1", "Alice", {
      isSessionParticipant: true,
    });
    const eventParticipant = option("human-3", "Carol", {
      isSessionParticipant: true,
    });
    const groups = buildSpeakerParticipantGroups({
      sessionParticipants: [participant],
      eventParticipants: [eventParticipant],
      contacts: [option("human-1", "Alice"), option("human-2", "Bob")],
      query: "",
    });

    expect(groups).toEqual([
      {
        title: "Participants",
        options: [participant, eventParticipant],
      },
      {
        title: "People",
        options: [option("human-2", "Bob")],
      },
    ]);
  });
});

describe("buildEventSpeakerParticipantOptions", () => {
  it("matches event participants to existing people by email", () => {
    expect(
      buildEventSpeakerParticipantOptions({
        eventParticipants: [{ name: "Alice A.", email: "alice@example.com" }],
        contacts: [option("human-1", "Alice", { email: "alice@example.com" })],
      }),
    ).toEqual([
      option("human-1", "Alice A.", {
        email: "alice@example.com",
        isSessionParticipant: true,
      }),
    ]);
  });

  it("creates pending participant options for event attendees without people", () => {
    expect(
      buildEventSpeakerParticipantOptions({
        eventParticipants: [{ name: "Bob", email: "bob@example.com" }],
        contacts: [],
      }),
    ).toEqual([
      option("event:bob@example.com", "Bob", {
        email: "bob@example.com",
        isSessionParticipant: true,
        isNew: true,
      }),
    ]);
  });

  it("does not match event attendees by name when their email differs", () => {
    expect(
      buildEventSpeakerParticipantOptions({
        eventParticipants: [{ name: "Bob", email: "bob@example.com" }],
        contacts: [option("human-1", "Bob", { email: "other@example.com" })],
      }),
    ).toEqual([
      option("event:bob@example.com", "Bob", {
        email: "bob@example.com",
        isSessionParticipant: true,
        isNew: true,
      }),
    ]);
  });

  it("keeps duplicate event attendees without emails selectable", () => {
    expect(
      buildEventSpeakerParticipantOptions({
        eventParticipants: [{ name: "Bob" }, { name: "Bob" }],
        contacts: [],
      }),
    ).toEqual([
      option("event:Bob:0", "Bob", {
        isSessionParticipant: true,
        isNew: true,
      }),
      option("event:Bob:1", "Bob", {
        isSessionParticipant: true,
        isNew: true,
      }),
    ]);
  });
});

describe("buildCreateSpeakerParticipantOption", () => {
  it("creates an add option for a new typed contact name", () => {
    expect(
      buildCreateSpeakerParticipantOption({
        query: "  Charlie  ",
        existingOptions: [option("human-1", "Alice")],
      }),
    ).toEqual({
      id: "new",
      name: "Charlie",
      isSessionParticipant: false,
      isNew: true,
      isCreateOption: true,
    });
  });

  it("does not create a duplicate add option", () => {
    expect(
      buildCreateSpeakerParticipantOption({
        query: "alice@example.com",
        existingOptions: [
          option("human-1", "Alice", { email: "alice@example.com" }),
        ],
      }),
    ).toBeNull();
  });
});

describe("getAssignmentAnchorWordId", () => {
  it("uses the first available word id in the segment", () => {
    const segment = {
      id: "segment-1",
      key: {
        channel: "RemoteParty",
        speaker_index: 1,
        speaker_human_id: null,
      },
      speaker_label: "Speaker 1",
      start_ms: 0,
      end_ms: 200,
      text: "hello there",
      words: [
        {
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: "RemoteParty",
          is_final: true,
        },
        {
          id: "word-2",
          text: "there",
          start_ms: 100,
          end_ms: 200,
          channel: "RemoteParty",
          is_final: true,
        },
      ],
    } as Segment;

    expect(getAssignmentAnchorWordId(segment)).toBe("word-2");
  });
});

describe("getAssignmentWordIds", () => {
  it("returns every persisted word id in the segment", () => {
    const segment = {
      id: "segment-1",
      key: {
        channel: "DirectMic",
        speaker_index: 1,
        speaker_human_id: null,
      },
      start_ms: 0,
      end_ms: 300,
      text: "hello there",
      words: [
        {
          id: "word-1",
          text: "hello",
          start_ms: 0,
          end_ms: 100,
          channel: "DirectMic",
          is_final: true,
        },
        {
          text: " ",
          start_ms: 100,
          end_ms: 120,
          channel: "DirectMic",
          is_final: true,
        },
        {
          id: "word-2",
          text: "there",
          start_ms: 120,
          end_ms: 300,
          channel: "DirectMic",
          is_final: true,
        },
      ],
    } as Segment;

    expect(getAssignmentWordIds(segment)).toEqual(["word-1", "word-2"]);
  });
});
