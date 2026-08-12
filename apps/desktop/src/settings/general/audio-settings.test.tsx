import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AudioSettingsView } from "./audio-settings";

function renderAudioSettings({
  microphoneDevice = {
    value: "",
    devices: ["External Microphone"],
    onChange: vi.fn(),
  },
} = {}) {
  return render(
    <AudioSettingsView
      audioRetention={{ value: "forever", onChange: vi.fn() }}
      microphoneDevice={microphoneDevice}
    />,
  );
}

describe("AudioSettingsView", () => {
  afterEach(cleanup);

  it("puts the microphone selector first and selects the system default", () => {
    renderAudioSettings();

    const controls = screen.getAllByRole("combobox");
    expect(controls[0]).toBe(
      screen.getByRole("combobox", { name: "Microphone" }),
    );
    expect(screen.getByText("Current default")).toBeTruthy();
  });

  it("shows when the selected microphone is unavailable and will fall back", () => {
    renderAudioSettings({
      microphoneDevice: {
        value: "Disconnected Microphone",
        devices: ["External Microphone"],
        onChange: vi.fn(),
      },
    });

    expect(
      screen.getByRole("combobox", { name: "Microphone" }).textContent,
    ).toContain(
      "Disconnected Microphone (Unavailable — using current default)",
    );
  });
});
