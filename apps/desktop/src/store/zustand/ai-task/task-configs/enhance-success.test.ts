import type { LanguageModel } from "ai";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { json2md } from "@hypr/editor/markdown";

import type { TaskConfig } from ".";
import { enhanceSuccess } from "./enhance-success";

import { useLiveTitle } from "~/store/zustand/live-title";

type EnhanceSuccessParams = Parameters<
  NonNullable<TaskConfig<"enhance">["onSuccess"]>
>[0];

function createTransformedArgs(): EnhanceSuccessParams["transformedArgs"] {
  return {
    language: "en",
    session: {
      title: "Weekly Review",
      startedAt: null,
      endedAt: null,
      event: null,
    },
    participants: [],
    template: null,
    preMeetingMemo: "",
    postMeetingMemo: "",
    transcripts: [],
    imageContext: [],
  };
}

function createParams(
  overrides: Partial<EnhanceSuccessParams> = {},
): EnhanceSuccessParams {
  const store = {
    setPartialRow: vi.fn(),
    getCell: vi.fn().mockReturnValue(""),
    getValue: vi.fn().mockReturnValue("user-1"),
    setRow: vi.fn(),
  } as unknown as EnhanceSuccessParams["store"];

  return {
    taskId: "note-1-enhance",
    text: "# Summary\n\n- Point",
    model: {} as LanguageModel,
    args: {
      sessionId: "session-1",
      enhancedNoteId: "note-1",
      templateId: undefined,
    },
    transformedArgs: createTransformedArgs(),
    store,
    settingsStore: {} as EnhanceSuccessParams["settingsStore"],
    signal: new AbortController().signal,
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("enhanceSuccess.onSuccess", () => {
  beforeEach(() => {
    useLiveTitle.setState({ titles: {} });
  });

  it("persists enhanced note content as ProseMirror JSON string", async () => {
    const params = createParams();

    await enhanceSuccess.onSuccess?.(params);

    expect(params.store.setPartialRow).toHaveBeenCalledWith(
      "enhanced_notes",
      "note-1",
      expect.objectContaining({
        content: expect.any(String),
      }),
    );

    const persisted = (params.store.setPartialRow as ReturnType<typeof vi.fn>)
      .mock.calls[0][2].content;
    expect(() => JSON.parse(persisted)).not.toThrow();
  });

  it("appends extracted meeting tags and stores session tag mappings", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue("Existing title"),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const params = createParams({
      store,
      text: "# Summary\n\nDiscussed launch plan #Launch.",
      transformedArgs: {
        ...createTransformedArgs(),
        preMeetingMemo: "Prep notes #prep #Launch",
        postMeetingMemo: "Follow up with #next_steps",
        template: {
          title: "Launch #template",
          description: null,
          sections: [
            {
              title: "Actions",
              description: "Track #owners",
            },
          ],
        },
      },
    });

    await enhanceSuccess.onSuccess?.(params);

    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[0][2].content;
    const markdown = json2md(JSON.parse(persisted)).trim();

    expect(markdown).toBe(
      "# Existing title\n\n# Summary\n\nDiscussed launch plan #Launch.\n\n#launch #prep #next_steps #template #owners",
    );
    expect(store.setRow).toHaveBeenCalledWith("tags", "launch", {
      user_id: "user-1",
      name: "launch",
    });
    expect(store.setRow).toHaveBeenCalledWith(
      "mapping_tag_session",
      "session-1:next_steps",
      {
        user_id: "user-1",
        tag_id: "next_steps",
        session_id: "session-1",
      },
    );
  });

  it("starts title generation when session title is empty", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(startTask).toHaveBeenCalledWith("session-1-title", {
      model: params.model,
      taskType: "title",
      args: {
        sessionId: "session-1",
        enhancedNote: "# Summary\n\n- Point",
        skipPersist: true,
      },
      onComplete: expect.any(Function),
    });
  });

  it("waits for the generated title before persisting summary content", async () => {
    let sessionTitle = "";
    const store = {
      setPartialRow: vi.fn((table, row, value) => {
        if (table === "sessions" && row === "session-1") {
          sessionTitle = value.title;
        }
      }),
      getCell: vi.fn((table, _row, cell) => {
        if (table === "sessions" && cell === "title") return sessionTitle;
        return "";
      }),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      config.onComplete?.("Positive Performance Feedback");
    });
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    const enhancedCallIndex = (
      store.setPartialRow as ReturnType<typeof vi.fn>
    ).mock.calls.findIndex(
      ([table, row]) => table === "enhanced_notes" && row === "note-1",
    );
    expect(enhancedCallIndex).toBeGreaterThan(-1);
    expect(startTask.mock.invocationCallOrder[0]).toBeLessThan(
      (store.setPartialRow as ReturnType<typeof vi.fn>).mock
        .invocationCallOrder[enhancedCallIndex],
    );

    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[enhancedCallIndex][2].content;
    expect(json2md(JSON.parse(persisted)).trim()).toBe(
      "# Positive Performance Feedback\n\n# Summary\n\n- Point",
    );

    expect(store.setPartialRow).toHaveBeenCalledWith("sessions", "session-1", {
      title: "Positive Performance Feedback",
    });
  });

  it("does not persist a generated title when summary content fails to persist", async () => {
    const store = {
      setPartialRow: vi.fn((table) => {
        if (table === "enhanced_notes") {
          throw new Error("write failed");
        }
      }),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      config.onComplete?.("Positive Performance Feedback");
    });
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(store.setPartialRow).not.toHaveBeenCalledWith(
      "sessions",
      "session-1",
      expect.objectContaining({
        title: "Positive Performance Feedback",
      }),
    );
  });

  it("does not write placeholder generated titles into summary content", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      config.onComplete?.("<EMPTY>");
    });
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[0][2].content;
    expect(json2md(JSON.parse(persisted)).trim()).toBe("# Summary\n\n- Point");
    expect(store.setPartialRow).not.toHaveBeenCalledWith(
      "sessions",
      "session-1",
      expect.objectContaining({
        title: "<EMPTY>",
      }),
    );
  });

  it("reuses a previous skipped title result when retrying summary persistence", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const params = createParams({
      store,
      getTaskState: vi.fn().mockReturnValue({
        taskType: "title",
        status: "success",
        streamedText: "Recovered Summary Title",
        abortController: null,
        currentStep: undefined,
      }),
    });

    await enhanceSuccess.onSuccess?.(params);

    expect(params.startTask).not.toHaveBeenCalled();
    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[0][2].content;
    expect(json2md(JSON.parse(persisted)).trim()).toBe(
      "# Recovered Summary Title\n\n# Summary\n\n- Point",
    );
    expect(store.setPartialRow).toHaveBeenCalledWith("sessions", "session-1", {
      title: "Recovered Summary Title",
    });
  });

  it("does not use a generated title if live title editing starts while waiting", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      useLiveTitle.getState().setTitle("session-1", "Custom title");
      config.onComplete?.("Generated Title");
    });
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[0][2].content;
    expect(json2md(JSON.parse(persisted)).trim()).toBe("# Summary\n\n- Point");
    expect(store.setPartialRow).not.toHaveBeenCalledWith(
      "sessions",
      "session-1",
      expect.objectContaining({ title: "Generated Title" }),
    );
  });

  it("does not persist summary content when cancelled during title generation", async () => {
    const abortController = new AbortController();
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue(""),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
      forEachRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockImplementation(async (_taskId, config) => {
      abortController.abort();
      config.onComplete?.("Generated Title");
    });
    const params = createParams({
      store,
      startTask,
      signal: abortController.signal,
    });

    await enhanceSuccess.onSuccess?.(params);

    expect(store.setPartialRow).not.toHaveBeenCalled();
  });

  it("does not start title generation when title already exists", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue("Existing title"),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const startTask = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ store, startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(startTask).not.toHaveBeenCalled();
  });

  it("stores the existing session title before generated summary sections", async () => {
    const store = {
      setPartialRow: vi.fn(),
      getCell: vi.fn().mockReturnValue("OpenCode Interface Type and GUI"),
      getValue: vi.fn().mockReturnValue("user-1"),
      setRow: vi.fn(),
    } as unknown as EnhanceSuccessParams["store"];
    const params = createParams({
      store,
      text: "# OpenCode Tool Discussion\n\n- Speakers discussed OpenCode.",
    });

    await enhanceSuccess.onSuccess?.(params);

    const persisted = (store.setPartialRow as ReturnType<typeof vi.fn>).mock
      .calls[0][2].content;
    expect(json2md(JSON.parse(persisted)).trim()).toBe(
      "# OpenCode Interface Type and GUI\n\n# OpenCode Tool Discussion\n\n- Speakers discussed OpenCode.",
    );
  });

  it("does not start title generation while the title is being edited", async () => {
    useLiveTitle.getState().setTitle("session-1", "Custom title");
    const startTask = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ startTask });

    await enhanceSuccess.onSuccess?.(params);

    expect(startTask).not.toHaveBeenCalled();
  });

  it("does not start title generation when title task is already running", async () => {
    const params = createParams({
      getTaskState: vi.fn().mockReturnValue({
        taskType: "title",
        status: "generating",
        streamedText: "",
        abortController: null,
        currentStep: undefined,
      }),
    });

    await enhanceSuccess.onSuccess?.(params);

    expect(params.startTask).not.toHaveBeenCalled();
  });
});
