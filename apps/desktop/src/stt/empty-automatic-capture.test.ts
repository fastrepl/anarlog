import { beforeEach, expect, it, vi } from "vitest";

import { discardEmptyAutomaticCapture } from "./empty-automatic-capture";

const mocks = vi.hoisted(() => ({
  speech: vi.fn(),
  remove: vi.fn(),
  execute: vi.fn(),
  empty: vi.fn(),
  flush: vi.fn(),
}));
vi.mock("@anlg/plugin-fs-sync", () => ({
  commands: {
    audioHasSpeech: mocks.speech,
    audioDelete: mocks.remove,
  },
}));
vi.mock("~/db", () => ({ liveQueryClient: { execute: mocks.execute } }));
vi.mock("~/session/queries", () => ({ isSessionEmpty: mocks.empty }));
vi.mock("~/session/editor-registry", () => ({
  flushSessionEditorChanges: mocks.flush,
}));
vi.mock("~/session/audio-operations", () => ({
  enqueueSessionAudioOperation: (_: string, operation: () => unknown) =>
    operation(),
}));

const input = {
  sessionId: "meeting",
  automatic: true,
  preserveExistingAudio: false,
  preserveExistingTranscript: false,
  initialTitle: "Standup",
  transcriptTouched: false,
  transcriptionComplete: true,
};

beforeEach(() => {
  vi.resetAllMocks();
  mocks.speech.mockResolvedValue({ status: "ok", data: false });
  mocks.remove.mockResolvedValue({ status: "ok", data: true });
  mocks.execute.mockResolvedValue([{ title: "Standup", has_attachments: 0 }]);
  mocks.empty.mockResolvedValue(true);
  mocks.flush.mockResolvedValue(undefined);
});

it.each([0, false])(
  "discards silent automatic audio when the attachment flag is %j",
  async (hasAttachments) => {
    mocks.execute.mockResolvedValue([
      { title: "Standup", has_attachments: hasAttachments },
    ]);
    expect(await discardEmptyAutomaticCapture(input)).toBe(true);
    expect(mocks.remove).toHaveBeenCalledWith("meeting");
    expect(mocks.execute).toHaveBeenCalledTimes(1);
    expect(mocks.execute.mock.calls[0][0]).toMatch(/^SELECT /);
  },
);

it.each([
  { automatic: false },
  { preserveExistingAudio: true },
  { preserveExistingTranscript: true },
  { transcriptTouched: true },
  { transcriptionComplete: false },
  { initialTitle: undefined },
])("keeps captures that cannot safely be discarded: %j", async (override) => {
  expect(await discardEmptyAutomaticCapture({ ...input, ...override })).toBe(
    false,
  );
  expect(mocks.speech).not.toHaveBeenCalled();
  expect(mocks.remove).not.toHaveBeenCalled();
});

it.each([
  { status: "ok", data: true },
  { status: "error", error: "decoder failed" },
])("keeps speech and uncertain analysis: %j", async (result) => {
  mocks.speech.mockResolvedValue(result);
  expect(await discardEmptyAutomaticCapture(input)).toBe(false);
  expect(mocks.remove).not.toHaveBeenCalled();
});

it("keeps recordings when the user adds notes", async () => {
  mocks.empty.mockResolvedValue(false);
  expect(await discardEmptyAutomaticCapture(input)).toBe(false);
  expect(mocks.remove).not.toHaveBeenCalled();
});

it.each([1, true, null, undefined, "0"])(
  "keeps audio when the attachment flag is present or uncertain: %j",
  async (hasAttachments) => {
    mocks.execute.mockResolvedValue([
      { title: "Standup", has_attachments: hasAttachments },
    ]);
    expect(await discardEmptyAutomaticCapture(input)).toBe(false);
    expect(mocks.remove).not.toHaveBeenCalled();
  },
);

it("flushes edits made during audio analysis before deciding whether to discard", async () => {
  mocks.speech.mockImplementation(async () => {
    mocks.flush.mockImplementation(async () => {
      mocks.empty.mockResolvedValue(false);
    });
    return { status: "ok", data: false };
  });
  expect(await discardEmptyAutomaticCapture(input)).toBe(false);
  expect(mocks.remove).not.toHaveBeenCalled();
});

it.each([{ rows: [{ title: "Renamed by the user" }] }, { rows: [] }])(
  "keeps renamed or deleted sessions",
  async ({ rows }) => {
    mocks.execute.mockResolvedValue(rows);
    expect(await discardEmptyAutomaticCapture(input)).toBe(false);
    expect(mocks.remove).not.toHaveBeenCalled();
  },
);
