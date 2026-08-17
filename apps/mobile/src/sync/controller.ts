export type MobileSyncPhase =
  | "inactive"
  | "starting"
  | "setup_required"
  | "ready"
  | "error"
  | "device_limit"
  | "identity_mismatch"
  | "not_entitled"
  | "reauth_required"
  | "account_mismatch";

export type MobileSyncSnapshot = {
  phase: MobileSyncPhase;
  running: boolean;
  syncingNow: boolean;
  hasUnsentChanges: boolean | null;
  lastSyncAtMs: number | null;
  errorMessage: string | null;
  consecutiveFailures: number;
};

export type MobileSyncSession = {
  apiUrl: string;
  accessToken: string;
  accountUserId: string;
};

type NativeSyncStatus = {
  configured: boolean;
  running: boolean;
  has_unsent_changes: boolean | null;
  last_sync_at_ms: number | null;
  last_error: string | null;
  consecutive_failures: number;
};

type ControllerDependencies = {
  readRecoveryKey: (accountUserId: string) => Promise<string | null>;
  saveRecoveryKey: (
    accountUserId: string,
    recoveryKey: string,
  ) => Promise<void>;
  deleteRecoveryKey: (accountUserId: string) => Promise<void>;
  generateRecoveryKey: () => Promise<string>;
  inspectRecoveryKey: (
    recoveryKey: string,
  ) => Promise<{ keyId: string; memberPublicKey: string }>;
  claimIdentity: (session: MobileSyncSession, keyId: string) => Promise<void>;
  getDevice: () => Promise<{
    fingerprint?: string | null;
    name?: string | null;
  }>;
  bootstrap: (
    session: MobileSyncSession,
    recoveryKey: string,
    device: { fingerprint?: string | null; name?: string | null },
  ) => Promise<"configured" | "account_mismatch">;
  stop: () => Promise<void>;
  syncNow: () => Promise<void>;
  getStatus: () => Promise<NativeSyncStatus>;
  reportError: (error: unknown, operation: string) => void;
};

type ControllerTimers = {
  setInterval: typeof setInterval;
  clearInterval: typeof clearInterval;
  setTimeout: typeof setTimeout;
  clearTimeout: typeof clearTimeout;
};

const initialSnapshot: MobileSyncSnapshot = {
  phase: "inactive",
  running: false,
  syncingNow: false,
  hasUnsentChanges: null,
  lastSyncAtMs: null,
  errorMessage: null,
  consecutiveFailures: 0,
};

const errorPhases = new Set<MobileSyncPhase>([
  "device_limit",
  "identity_mismatch",
  "not_entitled",
  "reauth_required",
]);

function errorPhase(error: unknown): MobileSyncPhase {
  if (!error || typeof error !== "object" || !("code" in error)) {
    return "error";
  }
  const code = (error as { code?: unknown }).code;
  return typeof code === "string" && errorPhases.has(code as MobileSyncPhase)
    ? (code as MobileSyncPhase)
    : "error";
}

function errorMessage(error: unknown): string {
  return error instanceof Error
    ? error.message
    : "Cloud sync is temporarily unavailable.";
}

export class MobileSyncController {
  private snapshot = initialSnapshot;
  private readonly listeners = new Set<() => void>();
  private session: MobileSyncSession | null = null;
  private generation = 0;
  private operationQueue: Promise<void> = Promise.resolve();
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly dependencies: ControllerDependencies;
  private readonly pollIntervalMs: number;
  private readonly retryDelayMs: number;
  private readonly timers: ControllerTimers;

  constructor(
    dependencies: ControllerDependencies,
    pollIntervalMs = 5_000,
    retryDelayMs = 15_000,
    timers: ControllerTimers = {
      setInterval,
      clearInterval,
      setTimeout,
      clearTimeout,
    },
  ) {
    this.dependencies = dependencies;
    this.pollIntervalMs = pollIntervalMs;
    this.retryDelayMs = retryDelayMs;
    this.timers = timers;
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  getSnapshot = (): MobileSyncSnapshot => this.snapshot;

  activate(session: MobileSyncSession): () => void {
    this.generation += 1;
    const generation = this.generation;
    this.session = session;
    this.clearTimers();
    this.update({ ...initialSnapshot, phase: "starting" });
    this.enqueue(async () => this.start(generation, session));
    return () => {
      if (this.generation === generation) {
        this.suspend();
      }
    };
  }

  suspend(): void {
    this.generation += 1;
    this.session = null;
    this.clearTimers();
    this.update(initialSnapshot);
    this.enqueue(async () => {
      await this.stopSafely();
    });
  }

  retry(): void {
    if (this.session) {
      this.activate(this.session);
    }
  }

  async generateRecoveryKey(): Promise<string> {
    const recoveryKey = await this.dependencies.generateRecoveryKey();
    await this.dependencies.inspectRecoveryKey(recoveryKey);
    return recoveryKey;
  }

  async confirmRecoveryKey(recoveryKey: string): Promise<void> {
    const session = this.requireSession();
    const identity = await this.dependencies.inspectRecoveryKey(recoveryKey);
    await this.storeAndClaimIdentity(session, recoveryKey, identity.keyId);
    this.activateIfCurrentAccount(session.accountUserId);
  }

  async importRecoveryKey(recoveryKey: string): Promise<void> {
    const session = this.requireSession();
    const normalized = recoveryKey.trim();
    const identity = await this.dependencies.inspectRecoveryKey(normalized);
    await this.storeAndClaimIdentity(session, normalized, identity.keyId);
    this.activateIfCurrentAccount(session.accountUserId);
  }

  async syncNow(): Promise<void> {
    if (this.snapshot.phase !== "ready" || this.snapshot.syncingNow) {
      return;
    }
    const generation = this.generation;
    this.update({ ...this.snapshot, syncingNow: true, errorMessage: null });
    try {
      await this.dependencies.syncNow();
      await this.refreshStatus(generation);
    } catch (error) {
      this.dependencies.reportError(error, "mobile_sync_now");
      if (generation === this.generation) {
        this.update({
          ...this.snapshot,
          errorMessage: errorMessage(error),
        });
      }
    } finally {
      if (generation === this.generation) {
        this.update({ ...this.snapshot, syncingNow: false });
      }
    }
  }

  private enqueue(operation: () => Promise<void>): void {
    this.operationQueue = this.operationQueue.then(operation, operation);
  }

  private async start(
    generation: number,
    session: MobileSyncSession,
  ): Promise<void> {
    await this.stopSafely();
    if (generation !== this.generation) return;

    try {
      const recoveryKey = await this.dependencies.readRecoveryKey(
        session.accountUserId,
      );
      if (generation !== this.generation) return;
      if (!recoveryKey) {
        this.update({ ...initialSnapshot, phase: "setup_required" });
        return;
      }

      const device = await this.dependencies.getDevice();
      if (generation !== this.generation) return;
      const result = await this.dependencies.bootstrap(
        session,
        recoveryKey,
        device,
      );
      if (generation !== this.generation) return;
      if (result === "account_mismatch") {
        this.update({ ...initialSnapshot, phase: "account_mismatch" });
        return;
      }

      this.update({ ...initialSnapshot, phase: "ready", running: true });
      await this.refreshStatus(generation);
      if (generation !== this.generation) return;
      this.startPolling(generation);
    } catch (error) {
      if (generation !== this.generation) return;
      const phase = errorPhase(error);
      this.dependencies.reportError(error, "mobile_sync_start");
      this.update({
        ...initialSnapshot,
        phase,
        errorMessage: errorMessage(error),
      });
      if (phase === "error") {
        this.scheduleRetry(generation);
      }
    }
  }

  private async refreshStatus(generation: number): Promise<void> {
    try {
      const status = await this.dependencies.getStatus();
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        phase: "ready",
        running: status.running,
        hasUnsentChanges: status.has_unsent_changes,
        lastSyncAtMs: status.last_sync_at_ms,
        errorMessage: status.last_error,
        consecutiveFailures: status.consecutive_failures,
      });
    } catch (error) {
      this.dependencies.reportError(error, "mobile_sync_status");
    }
  }

  private startPolling(generation: number): void {
    if (this.pollIntervalMs <= 0) return;
    this.pollTimer = this.timers.setInterval(() => {
      void this.refreshStatus(generation);
    }, this.pollIntervalMs);
  }

  private scheduleRetry(generation: number): void {
    if (this.retryDelayMs <= 0) return;
    this.retryTimer = this.timers.setTimeout(() => {
      if (generation === this.generation && this.session) {
        this.activate(this.session);
      }
    }, this.retryDelayMs);
  }

  private async stopSafely(): Promise<void> {
    try {
      await this.dependencies.stop();
    } catch (error) {
      this.dependencies.reportError(error, "mobile_sync_stop");
    }
  }

  private requireSession(): MobileSyncSession {
    if (!this.session) {
      throw new Error("Sign in to configure cloud sync.");
    }
    return this.session;
  }

  private async storeAndClaimIdentity(
    session: MobileSyncSession,
    recoveryKey: string,
    keyId: string,
  ): Promise<void> {
    const previousKey = await this.dependencies.readRecoveryKey(
      session.accountUserId,
    );
    await this.dependencies.saveRecoveryKey(session.accountUserId, recoveryKey);
    try {
      const currentSession = this.session;
      if (
        !currentSession ||
        currentSession.accountUserId !== session.accountUserId
      ) {
        throw new Error("The signed-in account changed during sync setup.");
      }
      await this.dependencies.claimIdentity(currentSession, keyId);
    } catch (error) {
      try {
        if (previousKey) {
          await this.dependencies.saveRecoveryKey(
            session.accountUserId,
            previousKey,
          );
        } else {
          await this.dependencies.deleteRecoveryKey(session.accountUserId);
        }
      } catch (rollbackError) {
        this.dependencies.reportError(
          rollbackError,
          "mobile_sync_identity_rollback",
        );
      }
      throw error;
    }
  }

  private activateIfCurrentAccount(accountUserId: string): void {
    if (this.session?.accountUserId === accountUserId) {
      this.activate(this.session);
    }
  }

  private clearTimers(): void {
    if (this.pollTimer) this.timers.clearInterval(this.pollTimer);
    if (this.retryTimer) this.timers.clearTimeout(this.retryTimer);
    this.pollTimer = null;
    this.retryTimer = null;
  }

  private update(snapshot: MobileSyncSnapshot): void {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener();
  }
}
