import { useForm } from "@tanstack/react-form";
import { CircleDotIcon, RotateCcwIcon, SaveIcon } from "lucide-react";
import { type ReactNode, useCallback, useMemo, useRef } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { cn } from "@hypr/utils";

import { PromptAssistantPanel } from "./assistant";
import { getDefaultPromptTemplate } from "./defaults";
import { PromptEditor, type PromptEditorHandle } from "./editor";
import { PromptInsertChip, PromptTemplatePreview } from "./preview";

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
  const customContent = main.UI.useCell(
    "prompts",
    selectedTask ?? "",
    "content",
    main.STORE_ID,
  );

  if (!selectedTask) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-neutral-500">
          Select a meeting-note task to inspect or customize its prompt.
        </p>
      </div>
    );
  }

  return (
    <PromptDetails
      key={`${selectedTask}:${customContent ?? "__default__"}`}
      selectedTask={selectedTask}
    />
  );
}

function PromptDetails({ selectedTask }: { selectedTask: TaskType }) {
  const store = main.UI.useStore(main.STORE_ID) as main.Store | undefined;
  const customContent = main.UI.useCell(
    "prompts",
    selectedTask,
    "content",
    main.STORE_ID,
  );
  const editorRef = useRef<PromptEditorHandle>(null);

  const taskConfig = TASK_CONFIGS.find(
    (config) => config.type === selectedTask,
  );
  const defaultTemplate = getDefaultPromptTemplate(selectedTask);
  const savedContent = customContent || defaultTemplate;
  const hasCustomPrompt = !!customContent;
  const variables = useMemo(
    () => [...(taskConfig?.variables ?? [])],
    [taskConfig?.variables],
  );
  const filters = useMemo(() => [...AVAILABLE_FILTERS], []);

  const form = useForm({
    defaultValues: {
      content: savedContent,
    },
    onSubmit: ({ value }) => {
      if (!store) {
        return;
      }

      const nextContent = value.content.trim();
      const normalizedDefault = defaultTemplate.trim();

      if (!nextContent || nextContent === normalizedDefault) {
        deleteCustomPrompt(store, selectedTask);
        return;
      }

      setCustomPrompt(store, selectedTask, nextContent);
    },
  });

  const handleInsertSnippet = useCallback((snippet: string) => {
    editorRef.current?.insertText(snippet);
  }, []);

  if (!taskConfig) {
    return null;
  }

  return (
    <form.Field name="content">
      {(field) => {
        const draftContent = field.state.value;
        const hasChanges = draftContent !== savedContent;

        return (
          <div className="flex h-full min-h-0 flex-col">
            <div className="border-b border-neutral-200 px-6 py-4">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <div className="flex items-center gap-2 text-xs text-neutral-500">
                    <CircleDotIcon className="h-3.5 w-3.5" />
                    <span>
                      {hasCustomPrompt ? "Custom override" : "Default behavior"}
                    </span>
                  </div>
                  <h2 className="mt-2 text-lg font-semibold text-neutral-900">
                    {taskConfig.label}
                  </h2>
                  <p className="mt-1 text-sm text-neutral-500">
                    {taskConfig.description}
                  </p>
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      field.handleChange(savedContent);
                      editorRef.current?.focus();
                    }}
                    disabled={!hasChanges}
                  >
                    <RotateCcwIcon className="h-3.5 w-3.5" />
                    Revert Draft
                  </Button>
                  {hasCustomPrompt ? (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => {
                        if (!store) {
                          return;
                        }

                        deleteCustomPrompt(store, selectedTask);
                      }}
                    >
                      Reset to Default
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    size="sm"
                    onClick={() => {
                      void form.handleSubmit();
                    }}
                    disabled={!hasChanges}
                  >
                    <SaveIcon className="h-3.5 w-3.5" />
                    Save
                  </Button>
                </div>
              </div>
            </div>

            <ResizablePanelGroup
              direction="horizontal"
              className="min-h-0 flex-1"
            >
              <ResizablePanel defaultSize={60} minSize={42}>
                <div className="flex h-full min-h-0 flex-col">
                  <div className="border-b border-neutral-200 px-6 py-4">
                    <div className="rounded-xl border border-neutral-200 bg-stone-50 px-4 py-3">
                      <p className="text-xs leading-5 text-neutral-600">
                        The built-in prompt uses internal helpers. This editor
                        works on the supported custom-Jinja surface that becomes
                        active after you save an override.
                      </p>
                    </div>

                    <div className="mt-4 flex flex-col gap-3">
                      <PromptLibraryRow
                        label="Variables"
                        helper="Click to insert or drag into the editor."
                      >
                        {variables.map((variable) => (
                          <PromptInsertChip
                            key={variable}
                            label={variable}
                            snippet={`{{ ${variable} }}`}
                            kind="variable"
                            onInsert={handleInsertSnippet}
                          />
                        ))}
                      </PromptLibraryRow>

                      <PromptLibraryRow
                        label="Filters"
                        helper="Use these inside an expression like {{ content | transcript }}."
                      >
                        {filters.map((filter) => (
                          <PromptInsertChip
                            key={filter}
                            label={filter}
                            snippet={`| ${filter}`}
                            kind="filter"
                            onInsert={handleInsertSnippet}
                          />
                        ))}
                      </PromptLibraryRow>
                    </div>
                  </div>

                  <ResizablePanelGroup
                    direction="vertical"
                    className="min-h-0 flex-1"
                  >
                    <ResizablePanel defaultSize={42} minSize={24}>
                      <PromptSection
                        title="Formatted View"
                        description="A cleaner read of the active draft. Inline chips can be dragged or inserted back into the editor."
                      >
                        <PromptTemplatePreview
                          content={draftContent}
                          onInsert={handleInsertSnippet}
                        />
                      </PromptSection>
                    </ResizablePanel>
                    <ResizableHandle />
                    <ResizablePanel defaultSize={58} minSize={32}>
                      <PromptSection
                        title="Template Source"
                        description="Edit the Jinja draft directly, or let Charlie rewrite it from the chat pane."
                        flush
                      >
                        <div className="min-h-0 flex-1 overflow-hidden rounded-xl border border-neutral-200">
                          <PromptEditor
                            ref={editorRef}
                            value={draftContent}
                            onChange={field.handleChange}
                            placeholder="Write a custom prompt override, drag chips into place, or ask Charlie to rewrite the draft."
                            variables={variables}
                            filters={filters}
                          />
                        </div>
                      </PromptSection>
                    </ResizablePanel>
                  </ResizablePanelGroup>
                </div>
              </ResizablePanel>

              <ResizableHandle />

              <ResizablePanel defaultSize={40} minSize={28}>
                <PromptAssistantPanel
                  selectedTask={selectedTask}
                  taskLabel={taskConfig.label}
                  taskDescription={taskConfig.description}
                  variables={variables}
                  filters={filters}
                  draftContent={draftContent}
                  hasCustomPrompt={hasCustomPrompt}
                  onApplyTemplate={(content) => {
                    field.handleChange(content);
                    editorRef.current?.focus();
                  }}
                />
              </ResizablePanel>
            </ResizablePanelGroup>
          </div>
        );
      }}
    </form.Field>
  );
}

function PromptLibraryRow({
  label,
  helper,
  children,
}: {
  label: string;
  helper: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-medium text-neutral-700">{label}</span>
        <span className="text-[11px] text-neutral-500">{helper}</span>
      </div>
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

function PromptSection({
  title,
  description,
  children,
  flush = false,
}: {
  title: string;
  description: string;
  children: ReactNode;
  flush?: boolean;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div
        className={cn([
          "border-b border-neutral-200 px-6 py-4",
          flush && "pb-3",
        ])}
      >
        <h3 className="text-sm font-medium text-neutral-900">{title}</h3>
        <p className="mt-1 text-xs leading-5 text-neutral-500">{description}</p>
      </div>
      <div className="flex min-h-0 flex-1 flex-col px-6 py-4">{children}</div>
    </div>
  );
}
