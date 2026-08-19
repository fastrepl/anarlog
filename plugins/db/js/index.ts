import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  GetMeetingInput,
  GetMeetingTranscriptInput as GeneratedGetMeetingTranscriptInput,
  GetRecurringMeetingHistoryInput as GeneratedGetRecurringMeetingHistoryInput,
  CloudsyncE2eeWitness,
  CloudsyncTokenConfigurationResult,
  CloudsyncWorkspaceProjection,
  CloudsyncWorkspaceKeyGrant,
  E2eeIdentityStatus,
  E2eeDeviceEnrollmentPackage,
  E2eeDeviceIdentity,
  E2eeRecoveryKeyIdentity,
  SealedWorkspaceE2eeKey,
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
  ListMeetingsInput as GeneratedListMeetingsInput,
  Meeting,
  MeetingPage,
  SessionIngestApplyResult,
  StartupStatus,
  SubscriptionRegistration,
  TranscriptPage,
  WorkspaceE2eeKeyRecipient,
} from "./bindings.gen";

export type {
  CloudsyncE2eeWitness,
  CloudsyncTokenConfigurationResult,
  CloudsyncWorkspaceProjection,
  CloudsyncWorkspaceKeyGrant,
  E2eeIdentityStatus,
  E2eeDeviceEnrollmentPackage,
  E2eeDeviceIdentity,
  E2eeRecoveryKeyIdentity,
  SealedWorkspaceE2eeKey,
  GetMeetingInput,
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
  Meeting,
  MeetingPage,
  SessionIngestApplyResult,
  StartupStatus,
  TranscriptPage,
  WorkspaceE2eeKeyRecipient,
} from "./bindings.gen";

export type ListMeetingsInput = Partial<GeneratedListMeetingsInput>;
export type GetMeetingTranscriptInput = Pick<
  GeneratedGetMeetingTranscriptInput,
  "meeting_id"
> &
  Partial<Omit<GeneratedGetMeetingTranscriptInput, "meeting_id">>;
export type GetRecurringMeetingHistoryInput = Pick<
  GeneratedGetRecurringMeetingHistoryInput,
  "meeting_id"
> &
  Partial<Omit<GeneratedGetRecurringMeetingHistoryInput, "meeting_id">>;

export type TransactionStatement = {
  sql: string;
  params: unknown[];
  expectedRowsAffected?: number;
};

export type CloudsyncAuth =
  | { type: "none" }
  | { type: "api_key"; api_key: string }
  | { type: "token"; token: string };

export type CloudsyncTableSpec = {
  table_name: string;
  crdt_algo?: string;
  init_flags?: number;
  enabled: boolean;
};

export type CloudsyncRuntimeConfig = {
  connection_string: string;
  auth: CloudsyncAuth;
  tables: CloudsyncTableSpec[];
  sync_interval_ms: number;
  wait_ms?: number;
  max_retries?: number;
};

export const CLOUDSYNC_ACTIVITY_DEFERRED_ERROR =
  "cloudsync_activity_deferred" as const;

export function isCloudsyncActivityDeferredError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  return message === CLOUDSYNC_ACTIVITY_DEFERRED_ERROR;
}

export type CloudsyncNetworkResult = {
  send?: {
    status: string;
    localVersion: number;
    serverVersion: number;
    lastFailure?: unknown;
  };
  receive?: {
    rows: number;
    tables: string[];
    error?: string;
    lastFailure?: unknown;
  };
};

export type CloudsyncActivityEntry = {
  timestamp_ms: number;
  trigger: "background" | "manual";
  status: "completed" | "progress" | "failed";
  sent_bytes: number;
  received_bytes: number;
  error: string | null;
};

export type CloudsyncStatus = {
  cloudsync_enabled: boolean;
  extension_loaded: boolean;
  configured: boolean;
  running: boolean;
  network_initialized: boolean;
  activity_paused: boolean;
  deferred_for_capture: boolean;
  last_sync: CloudsyncNetworkResult | null;
  last_sync_at_ms: number | null;
  has_unsent_changes: boolean | null;
  last_error: string | null;
  last_error_kind: "transient" | "auth" | "fatal" | null;
  consecutive_failures: number;
  recovery_pending?: boolean;
  recovery_delayed?: boolean;
  recovery_phase?:
    | "need_first_logout"
    | "need_barrier_insert"
    | "need_barrier_confirm"
    | "need_clean_receive"
    | "need_witness_repair"
    | "need_barrier_cleanup"
    | null;
  activity_log?: CloudsyncActivityEntry[];
};

export type QueryEvent<T = Record<string, unknown>> =
  | { event: "result"; data: T[] }
  | { event: "error"; data: string };

export async function listMeetings(
  input: ListMeetingsInput,
): Promise<MeetingPage> {
  return invoke("plugin:db|list_meetings", { input });
}

export async function getMeeting(input: GetMeetingInput): Promise<Meeting> {
  return invoke("plugin:db|get_meeting", { input });
}

export async function getMeetingTranscript(
  input: GetMeetingTranscriptInput,
): Promise<TranscriptPage> {
  return invoke("plugin:db|get_meeting_transcript", { input });
}

export async function getRecurringMeetingHistory(
  input: GetRecurringMeetingHistoryInput,
): Promise<MeetingPage> {
  return invoke("plugin:db|get_recurring_meeting_history", { input });
}

// Generic query path: returns named object rows for app-level SQL consumers.
export async function execute<T = Record<string, unknown>>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  return invoke("plugin:db|execute", { sql, params });
}

export async function executeTransaction(
  statements: TransactionStatement[],
): Promise<number[]> {
  return invoke("plugin:db|execute_transaction", { statements });
}

// Drizzle proxy path: returns raw positional rows in sqlite-proxy format.
export async function executeProxy(
  sql: string,
  params: unknown[] = [],
  method: "run" | "all" | "get" | "values",
): Promise<{ rows: unknown[] }> {
  return invoke("plugin:db|execute_proxy", { sql, params, method });
}

export async function getLegacyImportReport(): Promise<LegacyImportReport> {
  return invoke("plugin:db|get_legacy_import_report");
}

export async function getLegacyCleanupStatus(): Promise<LegacyCleanupStatus> {
  return invoke("plugin:db|get_legacy_cleanup_status");
}

export async function cleanupLegacyFiles(): Promise<LegacyCleanupResult> {
  return invoke("plugin:db|cleanup_legacy_files");
}

export async function runLegacyImport(dryRun = false): Promise<string> {
  return invoke("plugin:db|run_legacy_import", { dryRun });
}

export async function applySessionIngest(
  workspaceId: string,
  envelope: Record<string, unknown>,
): Promise<SessionIngestApplyResult> {
  return invoke("plugin:db|apply_session_ingest", { workspaceId, envelope });
}

export async function getE2eeIdentityStatus(
  accountUserId: string,
): Promise<E2eeIdentityStatus> {
  return invoke("plugin:db|get_e2ee_identity_status", { accountUserId });
}

export async function inspectE2eeRecoveryKey(
  recoveryKey: string,
): Promise<E2eeRecoveryKeyIdentity> {
  return invoke("plugin:db|inspect_e2ee_recovery_key", { recoveryKey });
}

export async function createE2eeIdentity(
  accountUserId: string,
): Promise<string> {
  return invoke("plugin:db|create_e2ee_identity", { accountUserId });
}

export async function importE2eeIdentity(
  accountUserId: string,
  recoveryKey: string,
): Promise<void> {
  return invoke("plugin:db|import_e2ee_identity", {
    accountUserId,
    recoveryKey,
  });
}

export async function getOrCreateE2eeDeviceIdentity(
  accountUserId: string,
): Promise<E2eeDeviceIdentity> {
  return invoke("plugin:db|get_or_create_e2ee_device_identity", {
    accountUserId,
  });
}

export async function sealE2eeRecoveryKeyForDevice(
  accountUserId: string,
  requestId: string,
  recipientPublicKey: string,
): Promise<E2eeDeviceEnrollmentPackage> {
  return invoke("plugin:db|seal_e2ee_recovery_key_for_device", {
    accountUserId,
    requestId,
    recipientPublicKey,
  });
}

export async function sealWorkspaceE2eeKeyForRecipients(
  accountUserId: string,
  workspaceId: string,
  recipients: WorkspaceE2eeKeyRecipient[],
  rotate: boolean,
  sourceGrant: CloudsyncWorkspaceKeyGrant | null = null,
): Promise<SealedWorkspaceE2eeKey> {
  return invoke("plugin:db|seal_workspace_e2ee_key_for_recipients", {
    accountUserId,
    workspaceId,
    recipients,
    rotate,
    sourceGrant,
  });
}

export async function importE2eeDeviceEnrollment(
  accountUserId: string,
  requestId: string,
  packageValue: E2eeDeviceEnrollmentPackage,
): Promise<E2eeRecoveryKeyIdentity> {
  return invoke("plugin:db|import_e2ee_device_enrollment", {
    accountUserId,
    requestId,
    package: packageValue,
  });
}

export async function configureCloudsync(
  config: CloudsyncRuntimeConfig,
): Promise<void> {
  return invoke("plugin:db|configure_cloudsync", {
    configJson: JSON.stringify(config),
  });
}

export async function configureCloudsyncToken(
  databaseId: string,
  token: string,
  workspaceId: string,
  e2eeWitness: CloudsyncE2eeWitness,
  workspaceProjection?: CloudsyncWorkspaceProjection,
  workspaceKeyGrants: CloudsyncWorkspaceKeyGrant[] = [],
): Promise<CloudsyncTokenConfigurationResult> {
  return invoke("plugin:db|configure_cloudsync_token", {
    databaseId,
    token,
    workspaceId,
    e2eeWitness,
    workspaceProjection: workspaceProjection ?? null,
    workspaceKeyGrants,
  });
}

export async function configureE2eeReplica(
  workspaceId: string,
  e2eeWitness: CloudsyncE2eeWitness,
): Promise<CloudsyncTokenConfigurationResult> {
  return invoke("plugin:db|configure_e2ee_replica", {
    workspaceId,
    e2eeWitness,
  });
}

export async function bindCloudsyncAccount(
  accountUserId: string,
): Promise<boolean> {
  return invoke("plugin:db|bind_cloudsync_account", { accountUserId });
}

export async function startCloudsync(): Promise<void> {
  return invoke("plugin:db|start_cloudsync");
}

export async function stopCloudsync(): Promise<void> {
  return invoke("plugin:db|stop_cloudsync");
}

export async function suspendCloudsync(): Promise<void> {
  return invoke("plugin:db|suspend_cloudsync");
}

export async function suspendCloudsyncForSignOut(): Promise<void> {
  return invoke("plugin:db|suspend_cloudsync_for_sign_out");
}

export async function suspendCloudsyncAfterAuthLoss(): Promise<void> {
  return invoke("plugin:db|suspend_cloudsync_after_auth_loss");
}

export async function getCloudsyncStatus(): Promise<CloudsyncStatus> {
  return invoke("plugin:db|get_cloudsync_status");
}

export async function waitUntilReady(): Promise<void> {
  return invoke("plugin:db|wait_until_ready");
}

export async function getStartupStatus(): Promise<StartupStatus> {
  return invoke("plugin:db|get_startup_status");
}

export async function beginCloudsyncActivity(
  activity: string,
  key: string,
): Promise<void> {
  return invoke("plugin:db|begin_cloudsync_activity", { activity, key });
}

export async function endCloudsyncActivity(
  activity: string,
  key: string,
): Promise<void> {
  return invoke("plugin:db|end_cloudsync_activity", { activity, key });
}

export async function syncCloudsyncNow(): Promise<CloudsyncNetworkResult> {
  return invoke("plugin:db|sync_cloudsync_now");
}

export async function subscribe<T = Record<string, unknown>>(
  sql: string,
  params: unknown[],
  options: {
    onData: (rows: T[]) => void;
    onError?: (error: string) => void;
  },
): Promise<() => Promise<void>> {
  const channel = new Channel<QueryEvent<T>>();

  channel.onmessage = (event) => {
    if (event.event === "result") {
      options.onData(event.data);
      return;
    }

    options.onError?.(event.data);
  };

  const registration: SubscriptionRegistration = await invoke(
    "plugin:db|subscribe",
    {
      sql,
      params,
      onEvent: channel,
    },
  );

  if (registration.analysis.kind === "non_reactive") {
    console.warn(
      `[plugin-db] live query subscription is non-reactive for SQL "${sql}": ${registration.analysis.data.reason}`,
    );
  }

  return async () => {
    channel.onmessage = () => {};
    await invoke("plugin:db|unsubscribe", { subscriptionId: registration.id });
  };
}
