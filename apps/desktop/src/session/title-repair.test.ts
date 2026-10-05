import { expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  title: "",
  contentFormat: "prosemirror_json",
  concurrentBody: null as string | null,
  body: JSON.stringify({
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 1 },
        content: [{ type: "text", text: "DEFCON 1" }],
      },
      { type: "paragraph", content: [{ type: "text", text: "Meeting notes" }] },
    ],
  }),
}));

vi.mock("./content-queries", () => ({
  loadSessionContentSnapshot: async () => ({
    title: state.title,
    event: { title: "DEFCON 1" },
    enhancedNotes: [
      {
        id: "summary-1",
        content: state.body,
        contentFormat: state.contentFormat,
      },
    ],
  }),
}));

vi.mock("./content-mutations", () => ({
  applyGeneratedSessionTitle: async (request: {
    currentTitle: string;
    nextTitle: string;
    documents: Array<{ currentContent: string; nextContent: string }>;
  }) => {
    if (state.concurrentBody !== null) {
      state.body = state.concurrentBody;
      state.concurrentBody = null;
      throw new Error("transaction statement 1 affected 0 rows; expected 1");
    }
    expect(request.currentTitle).toBe(state.title);
    expect(request.documents[0]?.currentContent).toBe(state.body);
    state.title = request.nextTitle;
    state.body = request.documents[0]!.nextContent;
  },
}));

import { repairMissingSessionTitle } from "./title-repair";

import { useLiveTitle } from "~/store/zustand/live-title";

it("recovers a missing calendar title without using section headings or overwriting edits", async () => {
  const body = state.body;
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("DEFCON 1");
  expect(state.body).toBe(body);

  state.title = "Renamed meeting";
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("Renamed meeting");

  state.title = "";
  state.body = body.replace("DEFCON 1", "Action Items");
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("");

  state.body = body.replace("DEFCON 1", "");
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("");

  state.body = body;
  useLiveTitle.getState().setTitle("session-1", "New draft");
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("");
  useLiveTitle.getState().clearTitle("session-1");

  state.contentFormat = "markdown";
  state.body = "# DEFCON 1\n\nMeeting notes";
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("DEFCON 1");
  expect(JSON.parse(state.body)).toMatchObject({
    type: "doc",
    content: [
      { type: "heading", content: [{ text: "DEFCON 1" }] },
      { type: "paragraph", content: [{ text: "Meeting notes" }] },
    ],
  });
});

it("retries title recovery against the latest note after a concurrent edit", async () => {
  state.title = "";
  state.contentFormat = "markdown";
  state.body = "# DEFCON 1\n\nOriginal notes";
  state.concurrentBody = "# DEFCON 1\n\nEdited elsewhere";
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("DEFCON 1");
  expect(JSON.parse(state.body).content[1].content[0].text).toBe(
    "Edited elsewhere",
  );

  state.title = "";
  state.body = "# DEFCON 1\n\nOriginal notes";
  state.contentFormat = "markdown";
  state.concurrentBody = "# Different meeting\n\nEdited elsewhere";
  await repairMissingSessionTitle("session-1", "summary-1");
  expect(state.title).toBe("");
  expect(state.body).toBe("# Different meeting\n\nEdited elsewhere");
});
