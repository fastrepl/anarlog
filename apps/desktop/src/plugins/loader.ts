import { convertFileSrc, invoke } from "@tauri-apps/api/core";

import { createPluginContext } from "./context";
import { getRegisteredPluginModule, setPluginDisplayName } from "./registry";
import type { PluginCleanup, PluginManifestEntry, PluginModule } from "./types";

let loadingPromise: Promise<void> | null = null;
const loadedPlugins = new Map<
  string,
  { module: PluginModule; cleanups: PluginCleanup[] }
>();
let beforeUnloadRegistered = false;

export function loadPlugins() {
  if (!loadingPromise) {
    loadingPromise = loadPluginsInner();
  }
  return loadingPromise;
}

export async function unloadPlugins() {
  const pluginIds = Array.from(loadedPlugins.keys());

  for (const pluginId of pluginIds) {
    await unloadPlugin(pluginId);
  }

  loadingPromise = null;
}

async function loadPluginsInner() {
  registerBeforeUnloadHandler();

  const plugins = await invoke<PluginManifestEntry[]>("list_plugins").catch(
    (error) => {
      console.error("Failed to list plugins", error);
      return [];
    },
  );

  for (const plugin of plugins) {
    setPluginDisplayName(plugin.id, plugin.name);

    const loaded = await loadPluginScript(plugin.mainPath).catch((error) => {
      console.error(`Failed to load plugin script: ${plugin.id}`, error);
      return false;
    });

    if (!loaded) {
      continue;
    }

    const module = getRegisteredPluginModule(plugin.id);
    if (!module) {
      console.error(`Plugin did not register itself: ${plugin.id}`);
      continue;
    }

    await unloadPlugin(plugin.id);

    const cleanups: PluginCleanup[] = [];
    const addCleanup = (cleanup: PluginCleanup) => {
      cleanups.push(cleanup);
    };

    const context = createPluginContext(plugin.id, addCleanup);
    const loadedModule = await Promise.resolve(module.onload(context))
      .then(() => module)
      .catch(async (error) => {
        console.error(`Plugin onload failed: ${plugin.id}`, error);
        await runCleanups(plugin.id, cleanups);
        return null;
      });

    if (!loadedModule) {
      continue;
    }

    loadedPlugins.set(plugin.id, { module: loadedModule, cleanups });
  }
}

function registerBeforeUnloadHandler() {
  if (beforeUnloadRegistered) {
    return;
  }

  beforeUnloadRegistered = true;
  window.addEventListener("beforeunload", () => {
    void unloadPlugins();
  });
}

async function unloadPlugin(pluginId: string) {
  const loadedPlugin = loadedPlugins.get(pluginId);
  if (!loadedPlugin) {
    return;
  }

  loadedPlugins.delete(pluginId);

  await runCleanups(pluginId, loadedPlugin.cleanups);

  await Promise.resolve(loadedPlugin.module.onunload?.()).catch((error) => {
    console.error(`Plugin onunload failed: ${pluginId}`, error);
  });
}

async function runCleanups(pluginId: string, cleanups: PluginCleanup[]) {
  for (const cleanup of [...cleanups].reverse()) {
    await Promise.resolve(cleanup()).catch((error) => {
      console.error(`Plugin cleanup failed: ${pluginId}`, error);
    });
  }
}

function loadPluginScript(path: string) {
  return new Promise<boolean>((resolve) => {
    const script = document.createElement("script");
    script.async = false;
    script.src = convertFileSrc(path);
    script.onload = () => resolve(true);
    script.onerror = () => resolve(false);
    document.head.appendChild(script);
  });
}
