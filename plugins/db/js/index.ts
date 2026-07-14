import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  GetMeetingInput,
  GetMeetingTranscriptInput as GeneratedGetMeetingTranscriptInput,
  GetRecurringMeetingHistoryInput as GeneratedGetRecurringMeetingHistoryInput,
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
  ListMeetingsInput as GeneratedListMeetingsInput,
  Meeting,
  MeetingPage,
  SubscriptionRegistration,
  TranscriptPage,
} from "./bindings.gen";

export type {
  GetMeetingInput,
  LegacyCleanupResult,
  LegacyCleanupStatus,
  LegacyImportReport,
  Meeting,
  MeetingPage,
  TranscriptPage,
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

export type CloudsyncStatus = {
  cloudsync_enabled: boolean;
  extension_loaded: boolean;
  configured: boolean;
  running: boolean;
  network_initialized: boolean;
  last_sync: CloudsyncNetworkResult | null;
  last_sync_at_ms: number | null;
  has_unsent_changes: boolean | null;
  last_error: string | null;
  last_error_kind: "transient" | "auth" | "fatal" | null;
  consecutive_failures: number;
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
): Promise<boolean> {
  return invoke("plugin:db|configure_cloudsync_token", {
    databaseId,
    token,
    workspaceId,
  });
}

export async function claimCloudsyncAccount(
  accountUserId: string,
): Promise<boolean> {
  return invoke("plugin:db|claim_cloudsync_account", { accountUserId });
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

export async function getCloudsyncStatus(): Promise<CloudsyncStatus> {
  return invoke("plugin:db|get_cloudsync_status");
}

export async function syncCloudsyncNow(): Promise<CloudsyncNetworkResult> {
  return invoke("plugin:db|sync_cloudsync_now");
}

export async function logoutCloudsync(
  discardUnsentChanges = false,
): Promise<void> {
  return invoke("plugin:db|logout_cloudsync", { discardUnsentChanges });
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
