import "../bridge";

import type {
  DrizzleProxyClient,
  LiveQueryClient,
  ProxyQueryMethod,
  ProxyQueryResult,
  Row,
} from "@hypr/db-runtime";

import { MobileDbBridge } from "../bridge";
import type { MobileDbBridgeInterface } from "../bridge";
import { getAppDbPath } from "../native/mobilePaths";

type CloudsyncOpenMode = "disabled" | "enabled";

let bridgePromise: Promise<MobileDbBridgeInterface> | null = null;
let bridgeMode: CloudsyncOpenMode | null = null;

export type CloudsyncAuth =
  | { type: "none" }
  | { type: "apiKey"; apiKey: string }
  | { type: "token"; token: string };

export type CloudsyncTableSpec = {
  tableName: string;
  crdtAlgo?: string;
  forceInit?: boolean;
  enabled: boolean;
};

export type CloudsyncRuntimeConfig = {
  connectionString: string;
  auth: CloudsyncAuth;
  tables: CloudsyncTableSpec[];
  syncIntervalMs: number;
  waitMs?: number;
  maxRetries?: number;
};

export type CloudsyncStatus = {
  open_mode: "disabled" | "enabled";
  extension_loaded: boolean;
  configured: boolean;
  running: boolean;
  network_initialized: boolean;
  last_sync_downloaded_count?: number;
  last_sync_at_ms?: number;
  has_unsent_changes?: boolean;
  last_error?: string;
  last_error_kind?: "transient" | "auth" | "fatal";
  consecutive_failures: number;
};

function toRustCloudsyncAuth(auth: CloudsyncAuth) {
  if (auth.type === "none") {
    return { type: "none" };
  }

  if (auth.type === "apiKey") {
    return { type: "api_key", api_key: auth.apiKey };
  }

  return { type: "token", token: auth.token };
}

function toRustCloudsyncConfig(config: CloudsyncRuntimeConfig) {
  return {
    connection_string: config.connectionString,
    auth: toRustCloudsyncAuth(config.auth),
    tables: config.tables.map((table) => ({
      table_name: table.tableName,
      crdt_algo: table.crdtAlgo,
      force_init: table.forceInit,
      enabled: table.enabled,
    })),
    sync_interval_ms: config.syncIntervalMs,
    wait_ms: config.waitMs,
    max_retries: config.maxRetries,
  };
}

async function openBridge(
  mode: CloudsyncOpenMode = "disabled",
): Promise<MobileDbBridgeInterface> {
  if (bridgePromise && bridgeMode !== mode) {
    const bridge = await bridgePromise;
    bridge.close();
    bridgePromise = null;
    bridgeMode = null;
  }

  if (!bridgePromise) {
    bridgeMode = mode;
    bridgePromise = getAppDbPath()
      .then((dbPath) => MobileDbBridge.open(dbPath, mode))
      .catch((error) => {
        bridgePromise = null;
        bridgeMode = null;
        throw error;
      });
  }

  return bridgePromise;
}

export async function closeBridge(): Promise<void> {
  if (!bridgePromise) {
    return;
  }

  const bridge = await bridgePromise;
  bridge.close();
  bridgePromise = null;
  bridgeMode = null;
}

export async function execute<T = Row>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  const bridge = await openBridge();
  return JSON.parse(bridge.execute(sql, JSON.stringify(params))) as T[];
}

export async function executeProxy(
  sql: string,
  params: unknown[] = [],
  method: ProxyQueryMethod,
): Promise<ProxyQueryResult> {
  const bridge = await openBridge();
  return JSON.parse(
    bridge.executeProxy(sql, JSON.stringify(params), method),
  ) as ProxyQueryResult;
}

export async function subscribe<T = Row>(
  sql: string,
  params: unknown[],
  options: {
    onData: (rows: T[]) => void;
    onError?: (error: string) => void;
  },
): Promise<() => void> {
  const bridge = await openBridge();
  const subscriptionId = bridge.subscribe(sql, JSON.stringify(params), {
    onResult: (rowsJson) => {
      options.onData(JSON.parse(rowsJson) as T[]);
    },
    onError: (message) => {
      options.onError?.(message);
    },
  });

  return () => {
    try {
      bridge.unsubscribe(subscriptionId);
    } catch {
      // Ignore duplicate unsubscribe or teardown races.
    }
  };
}

export const mobileLiveQueryClient: LiveQueryClient & DrizzleProxyClient = {
  execute,
  executeProxy,
  subscribe,
};

export async function cloudsyncVersion(): Promise<string> {
  const bridge = await openBridge("enabled");
  return bridge.cloudsyncVersion();
}

export async function cloudsyncInit(
  tableName: string,
  crdtAlgo?: string,
  force?: boolean,
): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.cloudsyncInit(tableName, crdtAlgo, force);
}

export async function cloudsyncNetworkInit(
  connectionString: string,
): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.cloudsyncNetworkInit(connectionString);
}

export async function cloudsyncNetworkSetApikey(apiKey: string): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.cloudsyncNetworkSetApikey(apiKey);
}

export async function cloudsyncNetworkSetToken(token: string): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.cloudsyncNetworkSetToken(token);
}

export async function cloudsyncNetworkSync(
  waitMs?: number,
  maxRetries?: number,
): Promise<number> {
  const bridge = await openBridge("enabled");
  return Number(
    bridge.cloudsyncNetworkSync(
      waitMs === undefined ? undefined : BigInt(waitMs),
      maxRetries === undefined ? undefined : BigInt(maxRetries),
    ),
  );
}

export async function configureCloudsync(
  config: CloudsyncRuntimeConfig,
): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.configureCloudsync(JSON.stringify(toRustCloudsyncConfig(config)));
}

export async function startCloudsync(): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.startCloudsync();
}

export async function stopCloudsync(): Promise<void> {
  const bridge = await openBridge("enabled");
  bridge.stopCloudsync();
}

export async function cloudsyncStatus(): Promise<CloudsyncStatus> {
  const bridge = await openBridge("enabled");
  return JSON.parse(bridge.cloudsyncStatus()) as CloudsyncStatus;
}

export async function cloudsyncSyncNow(): Promise<number> {
  const bridge = await openBridge("enabled");
  return Number(bridge.cloudsyncSyncNow());
}
