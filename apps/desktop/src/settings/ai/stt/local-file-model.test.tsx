import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const {
  inspectCustomModelPathMock,
  selectFileMock,
  setSettingValueMock,
  setSettingValuesMock,
  sonnerToastErrorMock,
  startServerForPathMock,
} = vi.hoisted(() => ({
  inspectCustomModelPathMock: vi.fn(),
  selectFileMock: vi.fn(),
  setSettingValueMock: vi.fn(),
  setSettingValuesMock: vi.fn(),
  sonnerToastErrorMock: vi.fn(),
  startServerForPathMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: selectFileMock,
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: {
    inspectCustomModelPath: inspectCustomModelPathMock,
    startServerForPath: startServerForPathMock,
  },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: {
    error: sonnerToastErrorMock,
  },
}));

vi.mock("~/settings/queries", () => ({
  setSettingValue: setSettingValueMock,
  setSettingValues: setSettingValuesMock,
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: () => "",
}));

import { LocalFileModel } from "./local-file-model";

afterEach(cleanup);

describe("LocalFileModel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    selectFileMock.mockResolvedValue("/models/ggml-small.bin");
    inspectCustomModelPathMock.mockResolvedValue({
      status: "ok",
      data: {
        path: "/models/ggml-small.bin",
        name: "ggml-small.bin",
        sizeBytes: 100,
        format: "ggml",
      },
    });
    startServerForPathMock.mockResolvedValue({
      status: "ok",
      data: "http://127.0.0.1:4040/v1",
    });
    setSettingValuesMock.mockResolvedValue(undefined);
  });

  test("validates and persists a selected whisper.cpp model", async () => {
    renderWithQueryClient(<LocalFileModel healthStatus={null} />);

    fireEvent.click(
      screen.getByRole("button", { name: "Choose a .bin model" }),
    );

    await waitFor(() =>
      expect(setSettingValuesMock).toHaveBeenCalledWith({
        current_stt_provider: "local_file",
        current_stt_model: "local-file",
        local_stt_model_path: "/models/ggml-small.bin",
      }),
    );
    expect(selectFileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [
          {
            name: "whisper.cpp models",
            extensions: ["bin", "gguf"],
          },
        ],
      }),
    );
    expect(startServerForPathMock).toHaveBeenCalledWith(
      "/models/ggml-small.bin",
    );
  });

  test("does not persist a model rejected by the backend", async () => {
    inspectCustomModelPathMock.mockResolvedValue({
      status: "error",
      error:
        "transcribe.cpp GGUF models are not supported yet. Select a whisper.cpp .bin model instead",
    });

    renderWithQueryClient(<LocalFileModel healthStatus={null} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Choose a .bin model" }),
    );

    await waitFor(() => expect(sonnerToastErrorMock).toHaveBeenCalled());
    expect(startServerForPathMock).not.toHaveBeenCalled();
    expect(setSettingValuesMock).not.toHaveBeenCalled();
  });
});

function renderWithQueryClient(children: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>,
  );
}
