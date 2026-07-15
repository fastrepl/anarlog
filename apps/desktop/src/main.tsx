import "./styles/globals.css";
import "./styles/cursor.css";

import * as Sentry from "@sentry/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode, useMemo } from "react";
import ReactDOM from "react-dom/client";
import { createManager } from "tinytick";
import {
  Provider as TinyTickProvider,
  useCreateManager,
} from "tinytick/ui-react";

import "@hypr/ui/globals.css";
import {
  getCurrentWebviewWindowLabel,
  init as initWindowsPlugin,
} from "@hypr/plugin-windows";
import { Toaster } from "@hypr/ui/components/ui/toast";

import { AITaskWindowSyncBridge } from "./ai/task-window-sync";
import { createToolRegistry } from "./contexts/tool-registry/core";
import { env } from "./env";
import { AppI18nProvider } from "./i18n/provider";
import { FloatingMeetingWindowHost } from "./meeting-float/host";
import { routeTree } from "./routeTree.gen";
import { EventListeners } from "./services/event-listeners";
import { TaskManager } from "./services/task-manager";
import { useRemoteSessionDeletionUndoListener } from "./session/hooks/useDeleteSession";
import { refreshLegacySettingsSnapshots } from "./settings/legacy-snapshots";
import { migratePlaintextAiProviderApiKeys } from "./settings/providers";
import { initializeApplicationSettings } from "./settings/queries";
import { initializeAppExitFlush } from "./shared/app-exit";
import { ErrorComponent, NotFoundComponent } from "./shared/control";
import { bootstrapThemeFromSettings } from "./shared/theme/apply";
import { AppThemeProvider } from "./shared/theme/provider";
import { createAITaskStore } from "./store/zustand/ai-task";
import { listenerStore } from "./store/zustand/listener/instance";

const toolRegistry = createToolRegistry();
const queryClient = new QueryClient();

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
    <AppThemeProvider>
      <AppI18nProvider>
        <AITaskWindowSyncBridge store={aiTaskStore} />
        <RouterProvider
          router={router}
          context={{
            listenerStore,
            aiTaskStore,
            toolRegistry,
          }}
        />
      </AppI18nProvider>
    </AppThemeProvider>
  );
}

if (env.VITE_SENTRY_DSN) {
  Sentry.init({
    dsn: env.VITE_SENTRY_DSN,
    release: env.VITE_APP_VERSION
      ? `hyprnote-desktop@${env.VITE_APP_VERSION}`
      : undefined,
    environment: import.meta.env.MODE,
    tracePropagationTargets: [],
    integrations: [Sentry.replayIntegration()],
    replaysSessionSampleRate: 0.1,
    replaysOnErrorSampleRate: 1.0,
  });
}

function AppRoot() {
  const manager = useCreateManager(() => {
    return createManager().start();
  });
  const isMainWindow = getCurrentWebviewWindowLabel() === "main";
  useRemoteSessionDeletionUndoListener(isMainWindow);

  return (
    <QueryClientProvider client={queryClient}>
      <TinyTickProvider manager={manager}>
        <App />
        {isMainWindow ? <TaskManager /> : null}
        {isMainWindow ? <FloatingMeetingWindowHost /> : null}
        {isMainWindow ? <EventListeners /> : null}
        <Toaster position="bottom-right" />
      </TinyTickProvider>
    </QueryClientProvider>
  );
}

initWindowsPlugin();

if (getCurrentWebviewWindowLabel() === "main") {
  void initializeAppExitFlush().catch((error) => {
    console.error("Failed to initialize the exit flush listener", error);
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
}

async function renderApp() {
  await refreshLegacySettingsSnapshots().catch((error) => {
    console.error("Failed to refresh legacy settings snapshots", error);
  });
  await initializeApplicationSettings().catch((error) => {
    console.error("Failed to initialize application settings", error);
  });
  await migratePlaintextAiProviderApiKeys().catch((error) => {
    console.error("Failed to migrate AI provider credentials", error);
  });
  await Promise.all([bootstrapThemeFromSettings(), enableReactScanInDev()]);
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
