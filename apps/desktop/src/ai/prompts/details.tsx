import { useCallback, useEffect, useState } from "react";

import { commands as templateCommands } from "@hypr/plugin-template";
import { PromptEditor } from "@hypr/tiptap/prompt";
import { Button } from "@hypr/ui/components/ui/button";

import * as main from "~/store/tinybase/store/main";
import {
  AVAILABLE_FILTERS,
  deleteCustomPrompt,
  setCustomPrompt,
  TASK_CONFIGS,
  type TaskType,
} from "~/store/tinybase/store/prompts";

export function PromptDetailsColumn({
  selectedTask,
}: {
  selectedTask: TaskType | null;
}) {
  if (!selectedTask) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-neutral-500">
          Select a task type to view or customize its prompt
        </p>
      </div>
    );
  }

  return <PromptDetails key={selectedTask} selectedTask={selectedTask} />;
}

function PromptDetails({ selectedTask }: { selectedTask: TaskType }) {
  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const customContent = main.UI.useCell(
    "prompts",
    selectedTask,
    "content",
    main.STORE_ID,
  );

  const [defaultContent, setDefaultContent] = useState("");
  const [localValue, setLocalValue] = useState(customContent || "");
  const [isLoading, setIsLoading] = useState(true);

  const taskConfig = TASK_CONFIGS.find((c) => c.type === selectedTask);
  const variables = taskConfig?.variables ?? [];

  useEffect(() => {
    setIsLoading(true);

    const template: Parameters<typeof templateCommands.render>[0] =
      selectedTask === "enhance"
        ? {
            enhanceUser: {
              session: {
                event: null,
                title: null,
                startedAt: null,
                endedAt: null,
              },
              participants: [],
              template: null,
              transcripts: [],
              preMeetingMemo: "",
              postMeetingMemo: "",
            },
          }
        : { titleUser: { enhancedNote: "" } };

    void templateCommands
      .render(template)
      .then((result) => {
        if (result.status === "ok") {
          setDefaultContent(result.data);
        }
      })
      .finally(() => {
        setIsLoading(false);
      });
  }, [selectedTask]);

  useEffect(() => {
    setLocalValue(customContent || "");
  }, [customContent, selectedTask]);

  const handleSave = useCallback(() => {
    if (!store) return;
    const trimmed = localValue.trim();
    if (trimmed) {
      setCustomPrompt(store, selectedTask, trimmed);
    } else {
      deleteCustomPrompt(store, selectedTask);
    }
  }, [store, selectedTask, localValue]);

  const handleReset = useCallback(() => {
    if (!store) return;
    deleteCustomPrompt(store, selectedTask);
    setLocalValue("");
  }, [store, selectedTask]);

  const hasChanges = localValue !== (customContent || "");
  const hasCustomPrompt = !!customContent;

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-neutral-200 px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold">{taskConfig?.label}</h2>
            <p className="mt-1 text-sm text-neutral-500">
              {taskConfig?.description}
            </p>
          </div>
          <div className="flex gap-2">
            {hasCustomPrompt && (
              <Button variant="outline" size="sm" onClick={handleReset}>
                Reset to Default
              </Button>
            )}
            <Button size="sm" onClick={handleSave} disabled={!hasChanges}>
              Save
            </Button>
          </div>
        </div>
      </div>

      <div className="border-b border-neutral-200 bg-neutral-50 px-6 py-3">
        <h3 className="mb-2 text-xs font-medium text-neutral-600">
          Available Variables
        </h3>
        <div className="flex flex-wrap gap-1.5">
          {variables.map((variable) => (
            <code
              key={variable}
              className="rounded-xs border border-neutral-200 bg-white px-2 py-0.5 font-mono text-xs"
            >
              {"{{ "}
              {variable}
              {" }}"}
            </code>
          ))}
        </div>
        <div className="mt-2 text-xs text-neutral-500">
          <span className="font-medium">Filters:</span>{" "}
          {AVAILABLE_FILTERS.map((filter, i) => (
            <span key={filter}>
              <code className="rounded-xs border border-neutral-200 bg-white px-1">
                {filter}
              </code>
              {i < AVAILABLE_FILTERS.length - 1 && ", "}
            </span>
          ))}
        </div>
      </div>

      <div className="flex flex-1 flex-col overflow-hidden">
        <div className="flex-1 p-6">
          <div className="h-full overflow-hidden rounded-lg border border-neutral-200">
            <PromptEditor
              value={localValue}
              onChange={setLocalValue}
              placeholder="Enter your custom prompt template using Jinja2 syntax..."
              variables={variables as string[]}
              filters={[...AVAILABLE_FILTERS]}
            />
          </div>
        </div>

        <div className="border-t border-neutral-200">
          <details className="group">
            <summary className="flex cursor-pointer list-none items-center gap-2 px-6 py-3 text-sm font-medium text-neutral-600 hover:bg-neutral-50">
              <svg
                className="h-4 w-4 transition-transform group-open:rotate-90"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M9 5l7 7-7 7"
                />
              </svg>
              Default Template Reference
            </summary>
            <div className="max-h-64 overflow-auto px-6 pb-4">
              {isLoading ? (
                <div className="text-sm text-neutral-500">Loading...</div>
              ) : (
                <pre className="rounded-lg border border-neutral-200 bg-neutral-50 p-4 font-mono text-xs whitespace-pre-wrap text-neutral-600">
                  {defaultContent || "No default template available"}
                </pre>
              )}
            </div>
          </details>
        </div>
      </div>
    </div>
  );
}
