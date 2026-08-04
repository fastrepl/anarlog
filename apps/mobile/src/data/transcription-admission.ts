export class TranscriptionAdmission {
  private active = 0;
  private readonly maxConcurrent: number;
  private readonly maxQueued: number;
  private readonly pending: Array<() => void> = [];

  constructor(maxConcurrent: number, maxQueued: number) {
    if (maxConcurrent < 1 || maxQueued < 0) {
      throw new Error("Invalid transcription admission limits");
    }
    this.maxConcurrent = maxConcurrent;
    this.maxQueued = maxQueued;
  }

  schedule(run: () => Promise<void>): Promise<void> | null {
    if (
      this.active >= this.maxConcurrent &&
      this.pending.length >= this.maxQueued
    ) {
      return null;
    }

    return new Promise<void>((resolve, reject) => {
      const start = () => {
        this.active += 1;
        void Promise.resolve()
          .then(run)
          .then(
            () => {
              this.finish();
              resolve();
            },
            (error: unknown) => {
              this.finish();
              reject(error);
            },
          );
      };

      if (this.active < this.maxConcurrent) {
        start();
      } else {
        this.pending.push(start);
      }
    });
  }

  private finish(): void {
    this.active -= 1;
    this.pending.shift()?.();
  }
}
