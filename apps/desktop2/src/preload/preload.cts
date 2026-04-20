import { contextBridge, ipcRenderer } from "electron";

import type {
  DrizzleProxyClient,
  LiveQueryClient,
  ProxyQueryMethod,
  ProxyQueryResult,
  QueryEvent,
  Row,
  Unsubscribe,
} from "@hypr/db-runtime";

import type { HyprElectronApi } from "../shared/api";
import { hyprIpcChannels } from "../shared/channels";
import type { DbSubscribeResult } from "../shared/subscribe";
import type { UpdaterEvent } from "../shared/updater";
import { updaterIpcChannels } from "../shared/updater";

const dbClient: DrizzleProxyClient & LiveQueryClient = {
  execute: <T = Row,>(sql: string, params: unknown[] = []) =>
    ipcRenderer.invoke(hyprIpcChannels.dbExecute, sql, params) as Promise<T[]>,

  executeProxy: (
    sql: string,
    params: unknown[],
    method: ProxyQueryMethod,
  ): Promise<ProxyQueryResult> =>
    ipcRenderer.invoke(
      hyprIpcChannels.dbExecuteProxy,
      sql,
      params,
      method,
    ) as Promise<ProxyQueryResult>,

  subscribe: async <T = Row,>(
    sql: string,
    params: unknown[],
    options: {
      onData: (rows: T[]) => void;
      onError?: (error: string) => void;
    },
  ): Promise<Unsubscribe> => {
    const { channel, reactive } = (await ipcRenderer.invoke(
      hyprIpcChannels.dbSubscribe,
      sql,
      params,
    )) as DbSubscribeResult;

    if (!reactive) {
      console.warn(
        `[desktop2] live query subscription is non-reactive for SQL "${sql}"`,
      );
    }

    const listener = (
      _event: Electron.IpcRendererEvent,
      delta: QueryEvent<T>,
    ) => {
      if (delta.event === "result") {
        options.onData(delta.data);
        return;
      }

      options.onError?.(delta.data);
    };
    ipcRenderer.on(channel, listener);

    return async () => {
      ipcRenderer.removeListener(channel, listener);
      await ipcRenderer.invoke(hyprIpcChannels.dbUnsubscribe, channel);
    };
  },
};

const api: HyprElectronApi = {
  db: dbClient,
  openExternal: (url) => ipcRenderer.invoke(hyprIpcChannels.openExternal, url),
  embeddedCli: {
    check: () => ipcRenderer.invoke(hyprIpcChannels.embeddedCliCheck),
    install: () => ipcRenderer.invoke(hyprIpcChannels.embeddedCliInstall),
    uninstall: () => ipcRenderer.invoke(hyprIpcChannels.embeddedCliUninstall),
  },
  updater: {
    check: () => ipcRenderer.invoke(updaterIpcChannels.check),
    install: () => ipcRenderer.invoke(updaterIpcChannels.install),
    subscribe: (callback) => {
      const listener = (
        _event: Electron.IpcRendererEvent,
        event: UpdaterEvent,
      ) => {
        callback(event);
      };
      ipcRenderer.on(updaterIpcChannels.event, listener);
      return () => {
        ipcRenderer.removeListener(updaterIpcChannels.event, listener);
      };
    },
  },
};

contextBridge.exposeInMainWorld("hypr", api);

// Renderer-side platform awareness. The UI uses this to apply a drag region
// and traffic-light padding on macOS without guessing from the user agent.
contextBridge.exposeInMainWorld("hyprPlatform", {
  os: process.platform,
});
