import "./styles/globals.css";
import "./styles/cursor.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode, useEffect, useMemo } from "react";
import ReactDOM from "react-dom/client";

import "@anlg/ui/globals.css";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import {
  getCurrentWebviewWindowLabel,
  init as initWindowsPlugin,
} from "@anlg/plugin-windows";
import { Toaster } from "@anlg/ui/components/ui/toast";

import { AITaskWindowSyncBridge } from "./ai/task-window-sync";
import { trackAnalyticsEvent } from "./analytics";
import { createToolRegistry } from "./contexts/tool-registry/core";
import {
  captureOperationalError,
  initializeErrorReporting,
} from "./error-reporting";
import { AppI18nProvider } from "./i18n/provider";
import { AppLockGate } from "./lock/gate";
import { FloatingMeetingWindowHost } from "./meeting-float/host";
import { routeTree } from "./routeTree.gen";
import { EventListeners } from "./services/event-listeners";
import { TaskManager } from "./services/task-manager";
import {
  createTaskScheduler,
  TaskSchedulerProvider,
} from "./services/task-scheduler";
import { TrayRecordingSync } from "./services/tray-recording";
import { TrayScheduleSync } from "./services/tray-schedule";
import { UpdaterMeetingSync } from "./services/updater-meeting";
import { useRemoteSessionDeletionUndoListener } from "./session/hooks/useDeleteSession";
import { refreshLegacySettingsSnapshots } from "./settings/legacy-snapshots";
import { migratePlaintextAiProviderApiKeys } from "./settings/providers";
import { initializeApplicationSettings } from "./settings/queries";
import { initializeAppExitFlush } from "./shared/app-exit";
import { initializeAppStoreBuild, isAppStoreBuild } from "./shared/app-store";
import { useConfigValue } from "./shared/config";
import { ErrorComponent, NotFoundComponent } from "./shared/control";
import { LongLoadGate } from "./shared/long-load-gate";
import { startInteractionProfiler } from "./shared/perf/interaction-profiler";
import { bootstrapThemeFromSettings } from "./shared/theme/apply";
import { AppThemeProvider } from "./shared/theme/provider";
import type { ThemePreference } from "./shared/theme/resolve";
import { createAITaskStore } from "./store/zustand/ai-task";
import { listenerStore } from "./store/zustand/listener/instance";

const toolRegistry = createToolRegistry();
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      gcTime: 60_000,
    },
  },
});
const STARTUP_TASK_TIMEOUT_MS = 10_000;

const router = createRouter({
  routeTree,
  context: undefined,
  defaultErrorComponent: ErrorComponent,
  defaultNotFoundComponent: NotFoundComponent,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

function App() {
  const aiTaskStore = useMemo(() => createAITaskStore(), []);

  return (
    <>
      <AITaskWindowSyncBridge store={aiTaskStore} />
      <RouterProvider
        router={router}
        context={{
          listenerStore,
          aiTaskStore,
          toolRegistry,
        }}
      />
    </>
  );
}

initializeErrorReporting();

function AppRoot() {
  return (
    <QueryClientProvider client={queryClient}>
      <LongLoadGate>
        <ReadyApp />
      </LongLoadGate>
    </QueryClientProvider>
  );
}

function ReadyApp() {
  const scheduler = useMemo(() => createTaskScheduler().start(), []);
  const theme = useConfigValue("theme") as ThemePreference;
  useRemoteSessionDeletionUndoListener(isMainWindow);

  useEffect(() => {
    runMainWindowStartupTasks();
  }, []);

  return (
    <AppThemeProvider>
      <AppI18nProvider>
        <TaskSchedulerProvider scheduler={scheduler}>
          <AppLockGate>
            <App />
            {isMainWindow ? <TaskManager /> : null}
            {isMainWindow ? <FloatingMeetingWindowHost /> : null}
            {isMainWindow ? <EventListeners /> : null}
            {isMainWindow ? <TrayScheduleSync /> : null}
            {isMainWindow ? <TrayRecordingSync /> : null}
            {isMainWindow && !isAppStoreBuild() ? <UpdaterMeetingSync /> : null}
            <Toaster position="bottom-right" theme={theme} />
          </AppLockGate>
        </TaskSchedulerProvider>
      </AppI18nProvider>
    </AppThemeProvider>
  );
}

initWindowsPlugin();

const isMainWindow = getCurrentWebviewWindowLabel() === "main";

if (isMainWindow) {
  void analyticsCommands.eventFireAndForget({ event: "app_started" });
  try {
    const firstOpenKey = "anarlog:analytics:first-opened";
    if (localStorage.getItem(firstOpenKey) === null) {
      localStorage.setItem(firstOpenKey, "1");
      trackAnalyticsEvent("app_first_opened", {
        first_open_marker: "local_install",
      });
    }
  } catch {}
  void initializeAppExitFlush().catch((error) => {
    captureOperationalError(error, {
      operation: "app_exit_flush_initialize",
    });
  });
}

const rootElement = document.getElementById("root")!;

async function enableReactScanInDev() {
  if (!import.meta.env.DEV) {
    return;
  }

  try {
    const { scan } = await import("react-scan");
    scan({ enabled: true });
  } catch (error) {
    console.warn("Failed to start React Scan:", error);
  }

  startInteractionProfiler();
}

async function renderApp() {
  await Promise.all([
    bootstrapThemeFromSettings(),
    enableReactScanInDev(),
    initializeAppStoreBuild(),
  ]);
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <AppRoot />
    </StrictMode>,
  );
}

if (!rootElement.innerHTML) {
  void renderApp();
}

function runMainWindowStartupTasks() {
  if (!isMainWindow) {
    return;
  }

  void runStartupTask(
    "legacy_settings_refresh",
    refreshLegacySettingsSnapshots,
  );
  void runStartupTask(
    "application_settings_initialize",
    initializeApplicationSettings,
  );
  void runStartupTask(
    "ai_credentials_migrate",
    migratePlaintextAiProviderApiKeys,
  );
}

async function runStartupTask(
  operation: string,
  task: () => Promise<void>,
): Promise<void> {
  let timeoutHandle: ReturnType<typeof setTimeout> | null = null;

  await new Promise<void>((resolve, reject) => {
    timeoutHandle = setTimeout(() => {
      reject(new Error(`startup operation timed out: ${operation}`));
    }, STARTUP_TASK_TIMEOUT_MS);

    void task().then(resolve, reject);
  })
    .catch((error) => {
      captureOperationalError(error, { operation });
    })
    .finally(() => {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }
    });
}
