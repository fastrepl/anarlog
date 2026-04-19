import "./styles/globals.css";

import * as Sentry from "@sentry/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode, useMemo } from "react";
import ReactDOM from "react-dom/client";
import { Provider as TinyBaseProvider, useStores } from "tinybase/ui-react";

import {
  getCurrentWebviewWindowLabel,
  init as initWindowsPlugin,
} from "@hypr/plugin-windows";
import { Toaster } from "@hypr/ui/components/ui/toast";
import "@hypr/ui/globals.css";

import { createToolRegistry } from "./contexts/tool-registry/core";
import { env } from "./env";
import { routeTree } from "./routeTree.gen";
import { CalendarSyncReconciler } from "./services/calendar-sync-reconciler";
import { EventListeners } from "./services/event-listeners";
import { EventNotificationManager } from "./services/event-notification-manager";
import { ErrorComponent, NotFoundComponent } from "./shared/control";
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
    return <div className="h-screen w-screen bg-stone-50" />;
  }

  return (
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
  const isMainWindow = getCurrentWebviewWindowLabel() === "main";

  return (
    <QueryClientProvider client={queryClient}>
      <TinyBaseProvider>
        <StoreComponent />
        <SettingsStoreComponent />
        <App />
        {isMainWindow ? <CalendarSyncReconciler /> : null}
        {isMainWindow ? <EventNotificationManager /> : null}
        {isMainWindow ? <EventListeners /> : null}
        <Toaster />
      </TinyBaseProvider>
    </QueryClientProvider>
  );
}

initWindowsPlugin();

const rootElement = document.getElementById("root")!;
if (!rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <AppRoot />
    </StrictMode>,
  );
}
