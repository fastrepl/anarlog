import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  clearTarget: vi.fn(),
  downloadModel: vi.fn(),
  toastError: vi.fn(),
  upgradeToPro: vi.fn(),
}));

vi.mock("@anlg/plugin-local-stt", () => ({
  commands: { downloadModel: mocks.downloadModel },
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.toastError },
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ upgradeToPro: mocks.upgradeToPro }),
}));

vi.mock("~/store/zustand/toast-action", () => ({
  useToastAction: (selector: (state: unknown) => unknown) =>
    selector({ target: null, clearTarget: mocks.clearTarget }),
}));

import { SttSettingsProvider, useSttSettings } from "./context";

function Probe() {
  const { queuedDownloads, startDownload } = useSttSettings();

  return (
    <>
      <div data-testid="queued">{queuedDownloads.join(",")}</div>
      <button onClick={() => startDownload("soniqo-parakeet-batch")}>
        Download
      </button>
    </>
  );
}

describe("SttSettingsProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    await waitFor(() => {
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Model download couldn’t start",
        { description: "batch model is unavailable" },
      );
    });
    expect(screen.getByTestId("queued").textContent).toBe("");
  });
});
