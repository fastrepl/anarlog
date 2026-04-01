import type {
  PluginCleanup,
  PluginContext as PluginContextBase,
  PluginEventRef,
  PluginModule as PluginModuleBase,
} from "@hypr/plugin-sdk";

import type { pluginEvents } from "./events";

export type PluginManifestEntry = {
  id: string;
  name: string;
  version: string;
  mainPath: string;
};

export type PluginEvents = typeof pluginEvents;

export type PluginContext = PluginContextBase<PluginEvents>;
export type PluginModule = PluginModuleBase<PluginEvents>;
export type { PluginCleanup, PluginEventRef };
