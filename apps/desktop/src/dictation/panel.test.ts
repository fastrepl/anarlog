import { beforeEach, describe, expect, it, vi } from "vitest";

import { waitForDictationPanel } from "./panel";
import { useDictationStatus } from "./state";

describe("dictation panel readiness", () => {
  beforeEach(() =>
    useDictationStatus.setState(useDictationStatus.getInitialState()),
  );

  it("waits for this recording's panel before allowing microphone capture", async () => {
    const ready = vi.fn();
    const promise = waitForDictationPanel(
      "current",
      new AbortController().signal,
    ).then(ready);
    useDictationStatus.setState({ presentedOwner: "previous" });
    await Promise.resolve();
    expect(ready).not.toHaveBeenCalled();
    useDictationStatus.setState({ presentedOwner: "current" });
    await promise;
    expect(ready).toHaveBeenCalledOnce();
  });

  it("rejects cancellation while the panel is opening", async () => {
    const abort = new AbortController();
    const promise = waitForDictationPanel("current", abort.signal);
    abort.abort();
    await expect(promise).rejects.toThrow("cancelled");
    useDictationStatus.setState({ presentedOwner: "current" });
  });
});
