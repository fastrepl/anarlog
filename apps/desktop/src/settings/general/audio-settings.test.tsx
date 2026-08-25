import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AudioSettingsView } from "./audio-settings";

function renderAudioSettings({
  microphoneDevice = {
    value: "",
    devices: ["External Microphone"],
    onChange: vi.fn(),
  },
  speakerDevice = {
    value: "",
    devices: ["External Speakers"],
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
      speakerDevice={speakerDevice}
      rememberSpeakers={rememberSpeakers}
    />,
  );
}

describe("AudioSettingsView", () => {
  afterEach(cleanup);

  it("puts the microphone and speaker selectors first and selects the system default", () => {
    renderAudioSettings();

    const controls = screen.getAllByRole("combobox");
    expect(controls[0]).toBe(
      screen.getByRole("combobox", { name: "Microphone" }),
    );
    expect(controls[1]).toBe(
      screen.getByRole("combobox", { name: "Speakers" }),
    );
    expect(screen.getAllByText("Current default")).toHaveLength(2);
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

  it("shows when the selected speakers are unavailable and will fall back", () => {
    renderAudioSettings({
      speakerDevice: {
        value: "Disconnected Speakers",
        devices: ["External Speakers"],
        onChange: vi.fn(),
      },
    });

    expect(
      screen.getByRole("combobox", { name: "Speakers" }).textContent,
    ).toContain("Disconnected Speakers (Unavailable — using current default)");
  });

  it("toggles remember speakers through its switch", () => {
    const onChange = vi.fn();
    renderAudioSettings({
      rememberSpeakers: { value: false, onChange },
    });

    const toggle = screen.getByRole("switch", { name: "Remember speakers" });
    expect(toggle.getAttribute("aria-checked")).toBe("false");

    toggle.click();
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
