import { useRouteContext } from "@tanstack/react-router";

import { TaskStorageProvider } from "@hypr/editor/task-storage";

import { ClassicMainServices } from "./lifecycle";

import { AITaskProvider } from "~/ai/contexts";
import { NotificationProvider } from "~/contexts/notifications";
import { ShellProvider } from "~/contexts/shell";
import { ToolRegistryProvider } from "~/contexts/tool";
import { useStoreBackedTaskStorage } from "~/editor-bridge/task-storage";
import { SearchEngineProvider } from "~/search/contexts/engine";
import { SearchUIProvider } from "~/search/contexts/ui";

export function ClassicMainLayout({
  children,
  includeServices = true,
}: {
  children: React.ReactNode;
  includeServices?: boolean;
}) {
  const { persistedStore, aiTaskStore, toolRegistry } = useRouteContext({
    from: "__root__",
  });
  const taskStorage = useStoreBackedTaskStorage();

  if (!aiTaskStore) {
    return null;
  }

  return (
    <SearchEngineProvider store={persistedStore}>
      <TaskStorageProvider storage={taskStorage}>
        <SearchUIProvider>
          <ShellProvider>
            <ToolRegistryProvider registry={toolRegistry}>
              <AITaskProvider store={aiTaskStore}>
                <NotificationProvider>
                  {includeServices ? <ClassicMainServices /> : null}
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
