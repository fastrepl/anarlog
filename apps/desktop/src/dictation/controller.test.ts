import { afterEach, describe, expect, it, vi } from "vitest";

import { DictationController } from "./controller";

function setup(handsFree = false) {
  const dependencies = {
    handsFree,
    start: vi.fn().mockResolvedValue(undefined),
    stop: vi.fn().mockResolvedValue("/tmp/dictation.wav"),
    cancel: vi.fn().mockResolvedValue(undefined),
    transcribe: vi.fn().mockResolvedValue("안녕하세요, world."),
    insert: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    onPhase: vi.fn(),
    onTranscript: vi.fn(),
    onError: vi.fn(),
  };
  return { ...dependencies, controller: new DictationController(dependencies) };
}

const settle = async (state: ReturnType<typeof setup>, phase = "idle") => {
  await vi.waitFor(() => expect(state.onPhase).toHaveBeenLastCalledWith(phase));
};

describe("system dictation", () => {
  afterEach(() => vi.useRealTimers());

  it("finishes a shortcut released while the microphone is still starting", async () => {
    const state = setup();
    state.controller.press();
    state.controller.release();
    await settle(state);
    expect(state.insert).toHaveBeenCalledWith("안녕하세요, world.");
    expect(state.discard).toHaveBeenCalledWith("/tmp/dictation.wav");
    expect(state.onPhase).toHaveBeenLastCalledWith("idle");
  });

  it("toggles hands-free recording and ignores key release", async () => {
    const state = setup(true);
    state.controller.press();
    state.controller.release();
    await settle(state, "recording");
    expect(state.stop).not.toHaveBeenCalled();
    state.controller.press();
    await settle(state);
    expect(state.insert).toHaveBeenCalledOnce();
  });

  it("cancels a pending microphone start without transcribing", async () => {
    const state = setup();
    state.controller.press();
    await state.controller.cancel();
    expect(state.cancel).toHaveBeenCalledOnce();
    expect(state.transcribe).not.toHaveBeenCalled();
    expect(state.insert).not.toHaveBeenCalled();
  });

  it("never cancels another recorder when its start was rejected", async () => {
    const state = setup();
    state.start.mockRejectedValueOnce(new Error("Already recording"));
    state.controller.press();
    await settle(state);
    expect(state.cancel).not.toHaveBeenCalled();
    expect(state.onError).toHaveBeenCalledOnce();
    expect(state.onPhase).toHaveBeenLastCalledWith("idle");
  });

  it("discards a late transcript after cancellation", async () => {
    const state = setup();
    let resolve!: (value: string) => void;
    state.transcribe.mockReturnValueOnce(
      new Promise<string>((done) => {
        resolve = done;
      }),
    );
    state.controller.press();
    state.controller.release();
    await vi.waitFor(() => expect(state.transcribe).toHaveBeenCalled());
    const cancel = state.controller.cancel();
    resolve("Do not insert this");
    await cancel;
    expect(state.insert).not.toHaveBeenCalled();
    expect(state.onTranscript).not.toHaveBeenCalled();
    expect(state.discard).toHaveBeenCalledOnce();
  });

  it("keeps the transcript recoverable when the focused field changed", async () => {
    const state = setup();
    state.insert.mockRejectedValueOnce(new Error("Focus changed"));
    state.controller.press();
    state.controller.release();
    await settle(state);
    expect(state.onTranscript).toHaveBeenCalledWith("안녕하세요, world.");
    expect(state.onError).toHaveBeenCalledOnce();
    expect(state.discard).toHaveBeenCalledOnce();
  });

  it("cleans up failed and silent transcriptions without inserting", async () => {
    for (const transcript of [
      Promise.reject(new Error("Network unavailable")),
      Promise.resolve("  "),
    ]) {
      const state = setup();
      state.transcribe.mockReturnValueOnce(transcript);
      state.controller.press();
      state.controller.release();
      await settle(state);
      expect(state.insert).not.toHaveBeenCalled();
      expect(state.discard).toHaveBeenCalledOnce();
      expect(state.onError).toHaveBeenCalledOnce();
    }
  });

  it("caps hands-free recording at five minutes", async () => {
    vi.useFakeTimers();
    const state = setup(true);
    state.controller.press();
    await settle(state, "recording");
    await vi.advanceTimersByTimeAsync(300_000);
    expect(state.stop).toHaveBeenCalledOnce();
    expect(state.insert).toHaveBeenCalledOnce();
  });

  it("ignores repeated presses and releases during transcription", async () => {
    const state = setup();
    let finish!: (text: string) => void;
    state.transcribe.mockReturnValueOnce(
      new Promise<string>((resolve) => {
        finish = resolve;
      }),
    );
    state.controller.press();
    await settle(state, "recording");
    state.controller.release();
    await vi.waitFor(() => expect(state.transcribe).toHaveBeenCalled());
    expect(state.onPhase).toHaveBeenLastCalledWith("transcribing");
    state.controller.press();
    state.controller.press();
    state.controller.release();
    state.controller.release();
    await settle(state, "transcribing");
    expect(state.start).toHaveBeenCalledOnce();
    expect(state.stop).toHaveBeenCalledOnce();
    expect(state.insert).not.toHaveBeenCalled();
    finish("Completed dictation");
    await settle(state);
    expect(state.insert).toHaveBeenCalledOnce();
  });
});
