import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { CameraWatermarkState } from "@anlg/plugin-camera-watermark";

const mocks = vi.hoisted(() => ({
  getState: vi.fn(),
  setEnabled: vi.fn(),
}));

vi.mock("@anlg/plugin-camera-watermark", () => ({
  commands: {
    getState: mocks.getState,
    setEnabled: mocks.setEnabled,
  },
  events: { cameraGuidanceEvent: { listen: vi.fn() } },
}));

import { CameraWatermarkSettings } from "./camera-watermark";

function cameraState(
  availability: CameraWatermarkState["availability"],
): CameraWatermarkState {
  return {
    prefs: { enabled: false, auto_switch: false, background: { kind: "none" } },
    availability,
    extension: "not_added",
    extension_failure: null,
    camera_access: "not_asked",
    agent_running: false,
    agent_registration: null,
    in_use_by: [],
    automation: {
      switches_camera: ["zoom", "teams", "webex"],
      removes_app_background: ["zoom", "teams"],
      manual_app_background: ["webex", "google_meet"],
    },
  };
}

function renderSettings(state?: CameraWatermarkState) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  if (state) {
    client.setQueryData(["camera-watermark", "state"], state);
  }
  return render(
    <QueryClientProvider client={client}>
      <CameraWatermarkSettings />
    </QueryClientProvider>,
  );
}

function cameraSwitch() {
  return screen.getByRole("switch", { name: "Anarlog Camera" });
}

afterEach(() => {
  cleanup();
  mocks.getState.mockReset();
  mocks.setEnabled.mockReset();
});

test("a build without Anarlog Camera does not offer camera settings", () => {
  const state = cameraState({ status: "unavailable", reason: "not_bundled" });
  mocks.getState.mockResolvedValue(state);
  renderSettings(state);

  expect(screen.queryByRole("switch", { name: "Anarlog Camera" })).toBeNull();
  expect(screen.queryByText("Switch my camera automatically")).toBeNull();
});

test("a refused opt-in leaves Anarlog Camera and its automatic switch off", async () => {
  mocks.getState.mockResolvedValue(cameraState({ status: "available" }));
  mocks.setEnabled.mockResolvedValue({
    status: "error",
    error: "Anarlog Camera isn't available in this build",
  });
  renderSettings();

  await waitFor(() => expect(cameraSwitch()).toHaveProperty("disabled", false));
  fireEvent.click(cameraSwitch());

  await screen.findByRole("alert");
  expect(cameraSwitch().getAttribute("aria-checked")).toBe("false");
  expect(
    screen
      .getByRole("switch", { name: "Switch my camera automatically" })
      .getAttribute("aria-checked"),
  ).toBe("false");
});
