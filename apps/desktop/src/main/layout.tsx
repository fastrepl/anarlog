import { useRouteContext } from "@tanstack/react-router";

import { ClassicMainServices } from "./lifecycle";

import { AITaskProvider } from "~/ai/contexts";
import { NotificationProvider } from "~/contexts/notifications";
import { ShellProvider } from "~/contexts/shell";
import { ToolRegistryProvider } from "~/contexts/tool";
import { TaskStorageProvider } from "~/editor/task-storage";
import { SearchEngineProvider } from "~/search/contexts/engine";
import { SearchUIProvider } from "~/search/contexts/ui";

export function ClassicMainLayout({ children }: { children: React.ReactNode }) {
  const { persistedStore, aiTaskStore, toolRegistry } = useRouteContext({
    from: "__root__",
  });

  if (!aiTaskStore) {
    return null;
  }

  return (
    <SearchEngineProvider store={persistedStore}>
      <TaskStorageProvider>
        <SearchUIProvider>
          <ShellProvider>
            <ToolRegistryProvider registry={toolRegistry}>
              <AITaskProvider store={aiTaskStore}>
                <NotificationProvider>
                  <ClassicMainServices />
                  {children}
                </NotificationProvider>
              </AITaskProvider>
            </ToolRegistryProvider>
          </ShellProvider>
        </SearchUIProvider>
      </TaskStorageProvider>
    </SearchEngineProvider>
  );
}
