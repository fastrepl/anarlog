export type DictationPhase = "idle" | "starting" | "recording" | "transcribing";

export class DictationController {
  private phase: DictationPhase = "idle";
  private cancelled = false;
  private released = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private operation: Promise<void> = Promise.resolve();

  constructor(
    private readonly dependencies: {
      start: () => Promise<void>;
      stop: () => Promise<string>;
      cancel: () => Promise<void>;
      transcribe: (path: string) => Promise<string>;
      insert: (text: string) => Promise<void>;
      discard: (path: string) => Promise<void>;
      onPhase: (phase: DictationPhase) => void;
      onTranscript: (text: string) => void;
      onError: (error: unknown) => void;
      handsFree: boolean;
    },
  ) {}

  press() {
    if (this.phase === "idle") {
      this.cancelled = false;
      this.released = false;
      this.setPhase("starting");
      this.operation = this.start();
    } else if (this.dependencies.handsFree && this.phase !== "transcribing") {
      this.release(true);
    }
  }

  release(force = false) {
    if (this.dependencies.handsFree && !force) return;
    if (this.phase === "starting") this.released = true;
    if (this.phase === "recording") {
      clearTimeout(this.timer);
      this.setPhase("transcribing");
      this.operation = this.finish();
    }
  }

  async cancel() {
    this.cancelled = true;
    clearTimeout(this.timer);
    if (this.phase === "recording") {
      // Keep the controller busy until this recording has relinquished the mic.
      this.setPhase("starting");
      this.operation = this.dependencies
        .cancel()
        .catch(this.dependencies.onError)
        .finally(() => this.setPhase("idle"));
    }
    await this.operation;
  }

  private setPhase(phase: DictationPhase) {
    this.phase = phase;
    this.dependencies.onPhase(phase);
  }

  private async start() {
    try {
      await this.dependencies.start();
      if (this.cancelled) {
        await this.dependencies.cancel();
        this.setPhase("idle");
        return;
      }
      this.setPhase("recording");
      this.timer = setTimeout(() => this.release(true), 300_000);
      if (this.released) {
        clearTimeout(this.timer);
        this.setPhase("transcribing");
        await this.finish();
      }
    } catch (error) {
      // A rejected start never owns the recorder (chat may already be using it).
      if (!this.cancelled) this.dependencies.onError(error);
      this.setPhase("idle");
    }
  }

  private async finish() {
    let path: string | undefined;
    try {
      path = await this.dependencies.stop();
      if (this.cancelled) return;
      const text = (await this.dependencies.transcribe(path)).trim();
      if (this.cancelled) return;
      if (!text) throw new Error("No speech detected. Please try again.");
      this.dependencies.onTranscript(text);
      await this.dependencies.insert(text);
    } catch (error) {
      if (!this.cancelled) this.dependencies.onError(error);
    } finally {
      if (path) {
        await this.dependencies.discard(path).catch(this.dependencies.onError);
      }
      this.setPhase("idle");
    }
  }
}
