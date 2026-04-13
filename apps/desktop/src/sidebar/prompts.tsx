import { CheckIcon, SparklesIcon } from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { TASK_CONFIGS } from "~/ai/prompts/config";
import { usePromptOverrides } from "~/ai/prompts/data";
import { useTabs } from "~/store/zustand/tabs";

export function PromptsNav() {
  const currentTab = useTabs((state) => state.currentTab);
  const updatePromptsTabState = useTabs((state) => state.updatePromptsTabState);

  const overridesQuery = usePromptOverrides();

  const selectedTask =
    currentTab?.type === "prompts" ? currentTab.state.selectedTask : null;

  const setSelectedTask = useCallback(
    (value: string | null) => {
      if (currentTab?.type === "prompts") {
        updatePromptsTabState(currentTab, {
          ...currentTab.state,
          selectedTask: value,
        });
      }
    },
    [currentTab, updatePromptsTabState],
  );

  if (currentTab?.type !== "prompts") {
    return null;
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <div className="flex h-12 items-center py-2 pr-1 pl-3">
        <h3 className="font-serif text-sm font-medium">Custom Prompts</h3>
      </div>
      <div className="scrollbar-hide flex-1 overflow-y-auto">
        <div className="p-2">
          {TASK_CONFIGS.map((config) => (
            <button
              key={config.type}
              onClick={() => setSelectedTask(config.type)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-neutral-100",
                selectedTask === config.type
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <SparklesIcon className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5 truncate font-medium">
                    {config.label}
                    {overridesQuery.data?.[config.type] && (
                      <span className="flex items-center gap-0.5 rounded-xs bg-green-100 px-1.5 py-0.5 text-xs text-green-700">
                        <CheckIcon className="h-3 w-3" />
                        Custom
                      </span>
                    )}
                  </div>
                  <div className="truncate text-xs text-neutral-500">
                    {config.description}
                  </div>
                </div>
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
