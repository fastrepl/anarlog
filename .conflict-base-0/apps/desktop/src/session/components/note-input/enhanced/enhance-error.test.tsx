import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  generate: vi.fn(),
  model: { modelId: "test-model" } as any,
  signIn: vi.fn(),
}));

vi.mock("~/ai/contexts", () => ({
  useAITask: (
    selector: (state: { generate: typeof mocks.generate }) => unknown,
  ) => selector({ generate: mocks.generate }),
}));

vi.mock("~/ai/hooks", () => ({
  useLanguageModel: () => mocks.model,
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ signIn: mocks.signIn }),
}));

vi.mock("~/session/queries", () => ({
  useEnhancedNote: () => ({ templateId: "template-1" }),
}));

vi.mock("~/store/zustand/ai-task/task-configs", () => ({
  createTaskId: () => "enhance-task",
}));

import { EnhanceError } from "./enhance-error";

function renderError(isUnauthenticated: boolean) {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <EnhanceError
        sessionId="session-1"
        enhancedNoteId="note-1"
        error={new Error("AI generation did not return any text.")}
        isUnauthenticated={isUnauthenticated}
      />
    </QueryClientProvider>,
  );
}

describe("EnhanceError", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.signIn.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it("explains that sign-in is required and opens sign-in", async () => {
    renderError(true);

    expect(screen.getByText("Sign in to generate this summary")).toBeTruthy();
    expect(
      screen.getByText(
        "Anarlog could not generate this summary because you were not signed in. Sign in, then try again.",
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText("AI generation did not return any text."),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));

    await waitFor(() => expect(mocks.signIn).toHaveBeenCalledOnce());
    expect(mocks.generate).not.toHaveBeenCalled();
  });

  it("keeps the retry action for other generation failures", () => {
    renderError(false);

    expect(screen.getByText("Summary generation failed")).toBeTruthy();
    expect(
      screen.getByText("AI generation did not return any text."),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    expect(mocks.generate).toHaveBeenCalledWith("enhance-task", {
      model: mocks.model,
      taskType: "enhance",
      args: {
        sessionId: "session-1",
        enhancedNoteId: "note-1",
        templateId: "template-1",
      },
    });
    expect(mocks.signIn).not.toHaveBeenCalled();
  });
});
