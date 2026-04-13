import { useForm } from "@tanstack/react-form";
import { CircleDotIcon, RotateCcwIcon, SaveIcon } from "lucide-react";
import { type ReactNode, useCallback, useMemo, useRef } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";

import { PromptAssistantPanel } from "./assistant";
import { AVAILABLE_FILTERS, TASK_CONFIGS, type TaskType } from "./config";
import {
  useDeletePromptOverrideMutation,
  usePromptOverride,
  usePromptTemplateSource,
  useUpsertPromptOverrideMutation,
} from "./data";
import { PromptEditor, type PromptEditorHandle } from "./editor";
import { PromptInsertChip, PromptTemplatePreview } from "./preview";

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

  return <PromptDetailsLoader selectedTask={selectedTask} />;
}

function PromptDetailsLoader({ selectedTask }: { selectedTask: TaskType }) {
  const overrideQuery = usePromptOverride(selectedTask);
  const templateQuery = usePromptTemplateSource(selectedTask);

  if (overrideQuery.error || templateQuery.error) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-red-600">Failed to load prompt data.</p>
      </div>
    );
  }

  if (
    overrideQuery.isLoading ||
    templateQuery.isLoading ||
    !templateQuery.data
  ) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-neutral-500">Loading prompt...</p>
      </div>
    );
  }

  return (
    <PromptDetails
      key={`${selectedTask}:${overrideQuery.data?.content ?? "__default__"}:${templateQuery.data}`}
      selectedTask={selectedTask}
      defaultSource={templateQuery.data}
      overrideContent={overrideQuery.data?.content ?? null}
    />
  );
}

function PromptDetails({
  selectedTask,
  defaultSource,
  overrideContent,
}: {
  selectedTask: TaskType;
  defaultSource: string;
  overrideContent: string | null;
}) {
  const editorRef = useRef<PromptEditorHandle>(null);
  const saveMutation = useUpsertPromptOverrideMutation(selectedTask);
  const resetMutation = useDeletePromptOverrideMutation(selectedTask);

  const taskConfig = TASK_CONFIGS.find(
    (config) => config.type === selectedTask,
  );
  const savedContent = overrideContent ?? defaultSource;
  const hasCustomPrompt = overrideContent !== null;
  const variables = useMemo(
    () => [...(taskConfig?.variables ?? [])],
    [taskConfig?.variables],
  );
  const filters = useMemo(() => [...AVAILABLE_FILTERS], []);

  const form = useForm({
    defaultValues: {
      content: savedContent,
    },
    onSubmit: async ({ value }) => {
      await saveMutation.mutateAsync(value.content.trim());
    },
  });

  const handleInsertSnippet = useCallback((snippet: string) => {
    editorRef.current?.insertText(snippet);
  }, []);

  if (!taskConfig) {
    return null;
  }

  const isMutating = saveMutation.isPending || resetMutation.isPending;

  return (
    <form.Field name="content">
      {(field) => {
        const draftContent = field.state.value;
        const hasChanges = draftContent !== savedContent;

        return (
          <ResizablePanelGroup
            direction="horizontal"
            className="h-full min-h-0"
          >
            <ResizablePanel defaultSize={60} minSize={42}>
              <div className="flex h-full min-h-0 flex-col">
                <div className="border-b border-neutral-200 px-6 py-3">
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <div className="flex items-center gap-2 text-xs text-neutral-500">
                        <CircleDotIcon className="h-3.5 w-3.5" />
                        <span>
                          {hasCustomPrompt
                            ? "Saved override"
                            : "Built-in template"}
                        </span>
                      </div>
                      <h2 className="mt-1 text-lg font-semibold text-neutral-900">
                        {taskConfig.label}
                      </h2>
                      <p className="mt-0.5 text-sm text-neutral-500">
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
                        disabled={!hasChanges || isMutating}
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
                            void resetMutation.mutateAsync();
                          }}
                          disabled={isMutating}
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
                        disabled={!hasChanges || isMutating}
                      >
                        <SaveIcon className="h-3.5 w-3.5" />
                        Save
                      </Button>
                    </div>
                  </div>
                </div>

                <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto">
                  <div className="flex flex-col gap-4 px-6 py-4">
                    <div className="rounded-xl border border-neutral-200 bg-stone-50 px-4 py-3">
                      <p className="text-xs leading-5 text-neutral-600">
                        This editor shows the real built-in Askama template when
                        no override is saved. Save to persist a SQLite-backed
                        override for this prompt task.
                      </p>
                    </div>

                    <div className="flex flex-col gap-3">
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

                    <PromptSection
                      title="Formatted View"
                      description="A cleaner read of the active draft. Inline chips can be dragged or inserted back into the editor."
                    >
                      <PromptTemplatePreview
                        content={draftContent}
                        onInsert={handleInsertSnippet}
                      />
                    </PromptSection>

                    <PromptSection
                      title="Template Source"
                      description="Edit the Jinja draft directly, or let Charlie rewrite it from the chat pane."
                    >
                      <div className="h-64 overflow-hidden rounded-xl border border-neutral-200">
                        <PromptEditor
                          ref={editorRef}
                          value={draftContent}
                          onChange={field.handleChange}
                          placeholder="Edit the template source, drag chips into place, or ask Charlie to rewrite the draft."
                          variables={variables}
                          filters={filters}
                        />
                      </div>
                    </PromptSection>
                  </div>
                </div>
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
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      <div>
        <h3 className="text-sm font-medium text-neutral-900">{title}</h3>
        <p className="mt-0.5 text-xs leading-5 text-neutral-500">
          {description}
        </p>
      </div>
      {children}
    </div>
  );
}
