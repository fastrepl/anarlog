import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AudioSettingsView } from "./audio-settings";

function renderAudioSettings({
  microphoneDevice = {
    value: "",
    devices: ["External Microphone"],
    onChange: vi.fn(),
  },
  rememberSpeakers = {
    value: false,
    onChange: vi.fn(),
  },
} = {}) {
  return render(
    <AudioSettingsView
      audioRetention={{ value: "forever", onChange: vi.fn() }}
      microphoneDevice={microphoneDevice}
      rememberSpeakers={rememberSpeakers}
    />,
  );
}

describe("AudioSettingsView", () => {
  afterEach(cleanup);

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

  it.each([
    [false, true],
    [true, false],
  ])("toggles remember speakers from %s to %s", (value, nextValue) => {
    const onChange = vi.fn();
    renderAudioSettings({
      rememberSpeakers: { value, onChange },
    });

    const toggle = screen.getByRole("switch", { name: "Remember speakers" });
    expect(toggle.getAttribute("aria-checked")).toBe(String(value));

    toggle.click();
    expect(onChange).toHaveBeenCalledWith(nextValue);
  });
});
