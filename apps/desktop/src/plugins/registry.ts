import type { PluginModule } from "./types";

const pluginModules = new Map<string, PluginModule>();
const pluginDisplayNames = new Map<string, string>();
const pluginViews = new Map<string, () => React.ReactNode>();

export function registerPluginModule(plugin: PluginModule) {
  pluginModules.set(plugin.id, plugin);
}

export function getRegisteredPluginModule(id: string) {
  return pluginModules.get(id);
}

export function setPluginDisplayName(pluginId: string, name: string) {
  pluginDisplayNames.set(pluginId, name);
}

export function getPluginDisplayName(pluginId: string) {
  return pluginDisplayNames.get(pluginId) ?? pluginId;
}

export function registerPluginView(
  pluginId: string,
  viewId: string,
  factory: () => React.ReactNode,
) {
  pluginViews.set(viewId, factory);
  if (!pluginDisplayNames.has(viewId)) {
    pluginDisplayNames.set(viewId, getPluginDisplayName(pluginId));
  }
}

export function getPluginView(viewId: string) {
  return pluginViews.get(viewId);
}
