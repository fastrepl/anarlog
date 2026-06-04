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
    startTask: vi.fn().mockResolvedValue(undefined),
    getTaskState: vi.fn().mockReturnValue(undefined),
    ...overrides,
  };
}

describe("enhanceSuccess.onSuccess", () => {
  beforeEach(() => {
    useLiveTitle.setState({ titles: {} });
  });

  it("persists enhanced note content as TipTap JSON string", async () => {
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
      "# Summary\n\nDiscussed launch plan #Launch.\n\n#launch #prep #next_steps #template #owners",
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
      args: { sessionId: "session-1" },
    });
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
