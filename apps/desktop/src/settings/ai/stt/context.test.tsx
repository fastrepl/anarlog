import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  downloadModel: vi.fn(),
  toastError: vi.fn(),
  upgradeToPro: vi.fn(),
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: { downloadModel: mocks.downloadModel },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.toastError },
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ upgradeToPro: mocks.upgradeToPro }),
}));

import { SttSettingsProvider, useSttSettings } from "./context";

import { usePendingSttSelection } from "~/store/zustand/pending-stt-selection";

function Probe() {
  const { queuedDownloads, startDownload } = useSttSettings();

  return (
    <>
      <div data-testid="queued">{queuedDownloads.join(",")}</div>
      <button onClick={() => startDownload("soniqo-parakeet-batch", "soniqo")}>
        Download
      </button>
    </>
  );
}

describe("SttSettingsProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    usePendingSttSelection.setState({ selection: null, queuedDownloads: [] });
  });

  it("shows the command error and makes a failed model retryable", async () => {
    mocks.downloadModel.mockResolvedValue({
      status: "error",
      error: "batch model is unavailable",
    });

    render(
      <SttSettingsProvider>
        <Probe />
      </SttSettingsProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(usePendingSttSelection.getState().selection).toEqual({
      provider: "soniqo",
      model: "soniqo-parakeet-batch",
    });

    await waitFor(() => {
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Model download couldn’t start",
        { description: "batch model is unavailable" },
      );
    });
    expect(screen.getByTestId("queued").textContent).toBe("");
    expect(usePendingSttSelection.getState().selection).toBeNull();
  });
});
