import "../bridge";

import type { LiveQueryClient, Row } from "@hypr/db-runtime";

import { MobileDbBridge, type MobileDbBridgeInterface } from "../bridge";
import { getAppDbPath } from "../native/mobilePaths";

let bridgePromise: Promise<MobileDbBridgeInterface> | null = null;

async function openBridge(): Promise<MobileDbBridgeInterface> {
  if (!bridgePromise) {
    bridgePromise = getAppDbPath()
      .then((dbPath) => MobileDbBridge.open(dbPath))
      .catch((error) => {
        bridgePromise = null;
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
}

export async function execute<T = Row>(
  sql: string,
  params: unknown[] = [],
): Promise<T[]> {
  const bridge = await openBridge();
  return JSON.parse(bridge.execute(sql, JSON.stringify(params))) as T[];
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

export const mobileLiveQueryClient: LiveQueryClient = {
  execute,
  subscribe,
};

export async function cloudsyncVersion(): Promise<string> {
  const bridge = await openBridge();
  return bridge.cloudsyncVersion();
}

export async function cloudsyncInit(
  tableName: string,
  crdtAlgo?: string,
  force?: boolean,
): Promise<void> {
  const bridge = await openBridge();
  bridge.cloudsyncInit(tableName, crdtAlgo, force);
}

export async function cloudsyncNetworkInit(
  connectionString: string,
): Promise<void> {
  const bridge = await openBridge();
  bridge.cloudsyncNetworkInit(connectionString);
}

export async function cloudsyncNetworkSetApikey(apiKey: string): Promise<void> {
  const bridge = await openBridge();
  bridge.cloudsyncNetworkSetApikey(apiKey);
}

export async function cloudsyncNetworkSetToken(token: string): Promise<void> {
  const bridge = await openBridge();
  bridge.cloudsyncNetworkSetToken(token);
}

export async function cloudsyncNetworkSync(
  waitMs?: number,
  maxRetries?: number,
): Promise<void> {
  const bridge = await openBridge();
  bridge.cloudsyncNetworkSync(
    waitMs === undefined ? undefined : BigInt(waitMs),
    maxRetries === undefined ? undefined : BigInt(maxRetries),
  );
}
