import { CheckIcon, SparklesIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";
import { TASK_CONFIGS, type TaskType } from "~/store/tinybase/store/prompts";

export function PromptsListColumn({
  selectedTask,
  setSelectedTask,
}: {
  selectedTask: TaskType | null;
  setSelectedTask: (id: string | null) => void;
}) {
  return (
    <div className="flex h-full w-full flex-col">
      <div className="flex h-12 items-center justify-between border-b border-neutral-200 py-2 pr-1 pl-3">
        <h3 className="text-sm font-medium">Custom Prompts</h3>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="p-2">
          {TASK_CONFIGS.map((config) => (
            <TaskItem
              key={config.type}
              taskType={config.type}
              label={config.label}
              description={config.description}
              isSelected={selectedTask === config.type}
              onClick={() => setSelectedTask(config.type)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function TaskItem({
  taskType,
  label,
  description,
  isSelected,
  onClick,
}: {
  taskType: TaskType;
  label: string;
  description: string;
  isSelected: boolean;
  onClick: () => void;
}) {
  const content = main.UI.useCell(
    "prompts",
    taskType,
    "content",
    main.STORE_ID,
  );
  const hasCustomPrompt = !!content;

  return (
    <button
      onClick={onClick}
      className={cn([
        "w-full rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-neutral-100",
        isSelected ? "border-neutral-500 bg-neutral-100" : "border-transparent",
      ])}
    >
      <div className="flex items-center gap-2">
        <SparklesIcon className="h-4 w-4 shrink-0 text-neutral-500" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5 truncate font-medium">
            {label}
            {hasCustomPrompt && (
              <span className="flex items-center gap-0.5 rounded-xs bg-green-100 px-1.5 py-0.5 text-xs text-green-700">
                <CheckIcon className="h-3 w-3" />
                Custom
              </span>
            )}
          </div>
          <div className="truncate text-xs text-neutral-500">{description}</div>
        </div>
      </div>
    </button>
  );
}
