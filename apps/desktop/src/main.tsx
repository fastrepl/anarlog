import "./styles/globals.css";
import "./styles/cursor.css";

import * as Sentry from "@sentry/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode, useMemo } from "react";
import ReactDOM from "react-dom/client";
import { Provider as TinyBaseProvider, useStores } from "tinybase/ui-react";
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
import { RawEditorSyncBridge } from "./session/raw-editor-sync";
import { ErrorComponent, NotFoundComponent } from "./shared/control";
import { bootstrapThemeFromSettings } from "./shared/theme/apply";
import { AppThemeProvider } from "./shared/theme/provider";
import {
  type Store,
  STORE_ID,
  StoreComponent,
} from "./store/tinybase/store/main";
import {
  STORE_ID as SETTINGS_STORE_ID,
  type Store as SettingsStore,
  StoreComponent as SettingsStoreComponent,
} from "./store/tinybase/store/settings";
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
  const stores = useStores();

  const store = stores[STORE_ID] as unknown as Store;
  const settingsStore = stores[SETTINGS_STORE_ID] as unknown as SettingsStore;

  const aiTaskStore = useMemo(() => {
    if (!store || !settingsStore) {
      return null;
    }
    return createAITaskStore({ persistedStore: store, settingsStore });
  }, [store, settingsStore]);

  if (!store || !settingsStore || !aiTaskStore) {
    return <div className="bg-background h-screen w-screen" />;
  }

  return (
    <AppThemeProvider>
      <AppI18nProvider>
        <AITaskWindowSyncBridge store={aiTaskStore} />
        <RouterProvider
          router={router}
          context={{
            persistedStore: store,
            internalStore: store,
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

function AppWithTiny() {
  const manager = useCreateManager(() => {
    return createManager().start();
  });
  const isMainWindow = getCurrentWebviewWindowLabel() === "main";
  useRemoteSessionDeletionUndoListener(isMainWindow);

  return (
    <QueryClientProvider client={queryClient}>
      <TinyTickProvider manager={manager}>
        <TinyBaseProvider>
          <StoreComponent />
          <SettingsStoreComponent />
          <RawEditorSyncBridge />
          <App />
          {isMainWindow ? <TaskManager /> : null}
          {isMainWindow ? <FloatingMeetingWindowHost /> : null}
          {isMainWindow ? <EventListeners /> : null}
          <Toaster />
        </TinyBaseProvider>
      </TinyTickProvider>
    </QueryClientProvider>
  );
}

initWindowsPlugin();

const rootElement = document.getElementById("root")!;
async function renderApp() {
  await bootstrapThemeFromSettings();
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <AppWithTiny />
    </StrictMode>,
  );
}

if (!rootElement.innerHTML) {
  void renderApp();
}
