import { Trans, useLingui } from "@lingui/react/macro";
import { useMemo, useState } from "react";

import { Lightning, MagnifyingGlass, Plus, X } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import {
  useDeleteChatAutomation,
  useDeleteWorkflow,
  useRemoveStarterDraft,
} from "~/automations/actions";
import {
  useAutomationSelection,
  useEffectiveAutomationSelection,
} from "~/automations/selection";
import {
  type StarterAutomation,
  useStarterAutomations,
} from "~/automations/starters";
import {
  type AutomationWorkflow,
  createEmptyWorkflow,
  saveAutomationWorkflows,
  useAutomationWorkflows,
} from "~/automations/workflows";
import { type ChatGroupRecord, useChatGroups } from "~/chat/store/queries";
import {
  importSharedAutomation,
  parseSharedAutomationPayload,
  SharedResourceLibrarySection,
} from "~/resource-sharing";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

export function AutomationsNav() {
  const { t } = useLingui();
  const [search, setSearch] = useState("");
  const searchRef = useSquircleRef<HTMLDivElement>();
  const starters = useStarterAutomations();
  const chatAutomations = useChatGroups("automations");
  const workflows = useAutomationWorkflows();
  const selection = useEffectiveAutomationSelection();
  const draftIds = useAutomationSelection((state) => state.draftIds);
  const selectStarter = useAutomationSelection((state) => state.selectStarter);
  const selectChatAutomation = useAutomationSelection(
    (state) => state.selectChatAutomation,
  );
  const selectWorkflow = useAutomationSelection(
    (state) => state.selectWorkflow,
  );

  const query = search.trim().toLowerCase();
  const draftTitle = t`Untitled automation`;
  const filteredStarters = query
    ? starters.filter(
        (starter) =>
          starter.title.toLowerCase().includes(query) ||
          starter.description.toLowerCase().includes(query),
      )
    : starters;
  const filteredDraftIds =
    query && !draftTitle.toLowerCase().includes(query) ? [] : draftIds;
  const chatIdsWithWorkflow = useMemo(
    () =>
      new Set(
        workflows
          .map((workflow) => workflow.chatGroupId)
          .filter((groupId): groupId is string => Boolean(groupId)),
      ),
    [workflows],
  );
  const filteredWorkflows = useMemo(() => {
    if (!query) {
      return workflows;
    }

    return workflows.filter((workflow) =>
      workflow.title.toLowerCase().includes(query),
    );
  }, [query, workflows]);
  const filteredChatAutomations = useMemo(() => {
    const orphans = chatAutomations.filter(
      (automation) => !chatIdsWithWorkflow.has(automation.id),
    );
    if (!query) {
      return orphans;
    }

    return orphans.filter((automation) =>
      automation.title.toLowerCase().includes(query),
    );
  }, [chatAutomations, chatIdsWithWorkflow, query]);
  const isEmpty =
    filteredStarters.length === 0 &&
    filteredDraftIds.length === 0 &&
    filteredWorkflows.length === 0 &&
    filteredChatAutomations.length === 0;

  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <CustomSidebarHeader>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          className="text-muted-foreground hover:text-foreground relative z-[60]"
          aria-label={t`New automation`}
          onClick={() => {
            const workflow = createEmptyWorkflow();
            void saveAutomationWorkflows([workflow, ...workflows]).then(() => {
              selectWorkflow(workflow.id);
            });
          }}
        >
          <Plus size={16} />
        </Button>
      </CustomSidebarHeader>

      <div className="pb-2">
        <div
          ref={searchRef}
          className={cn([
            "border-border bg-accent/50 flex h-8 w-full shrink-0 items-center gap-2 rounded-lg border px-3",
            "focus-within:bg-accent transition-colors",
          ])}
        >
          <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder={t`Search automations...`}
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              className={cn([
                "size-4 shrink-0",
                "text-muted-foreground hover:text-foreground",
                "transition-colors",
              ])}
              aria-label={t`Clear search`}
            >
              <X className="size-4" />
            </button>
          ) : null}
        </div>
      </div>

      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto pt-1">
        {isEmpty ? (
          <div className="text-muted-foreground px-3 py-8 text-center">
            <Lightning
              size={32}
              className="text-muted-foreground/70 mx-auto mb-2"
            />
            <p className="text-sm">
              {search ? (
                <Trans>No automations found</Trans>
              ) : (
                <Trans>No automations yet</Trans>
              )}
            </p>
          </div>
        ) : (
          <>
            {filteredStarters.length > 0 ? (
              <div className="pb-2">
                <h3 className="text-muted-foreground px-3 pt-1 pb-1 text-xs font-medium">
                  <Trans>Get started</Trans>
                </h3>
                {filteredStarters.map((starter) => (
                  <StarterListItem
                    key={starter.id}
                    starter={starter}
                    selected={
                      selection?.kind === "starter" &&
                      selection.starterId === starter.id
                    }
                    onSelect={selectStarter}
                  />
                ))}
              </div>
            ) : null}

            {filteredDraftIds.length > 0 ||
            filteredWorkflows.length > 0 ||
            filteredChatAutomations.length > 0 ? (
              <div>
                <h3 className="text-muted-foreground px-3 pt-1 pb-1 text-xs font-medium">
                  <Trans>My automations</Trans>
                </h3>
                {filteredDraftIds.map((draftId) => (
                  <DraftListItem
                    key={draftId}
                    draftId={draftId}
                    title={draftTitle}
                    selected={
                      selection?.kind === "draft" &&
                      selection.draftId === draftId
                    }
                  />
                ))}
                {filteredWorkflows.map((workflow) => (
                  <WorkflowListItem
                    key={workflow.id}
                    workflow={workflow}
                    selected={
                      selection?.kind === "workflow" &&
                      selection.workflowId === workflow.id
                    }
                    onSelect={selectWorkflow}
                  />
                ))}
                {filteredChatAutomations.map((automation) => (
                  <ChatAutomationListItem
                    key={automation.id}
                    automation={automation}
                    selected={
                      selection?.kind === "chat" &&
                      selection.groupId === automation.id
                    }
                    onSelect={selectChatAutomation}
                  />
                ))}
              </div>
            ) : null}
            <SharedResourceLibrarySection
              resourceType="automation"
              search={search}
              onImport={async (resource) => {
                const workflow = parseSharedAutomationPayload(resource.payload);
                const workflowId = await importSharedAutomation({
                  version: 1,
                  workflow,
                });
                selectWorkflow(workflowId);
              }}
            />
          </>
        )}
      </div>
    </div>
  );
}

function StarterListItem({
  starter,
  selected,
  onSelect,
}: {
  starter: StarterAutomation;
  selected: boolean;
  onSelect: (starterId: StarterAutomation["id"]) => void;
}) {
  const { t } = useLingui();
  const removeStarterDraft = useRemoveStarterDraft();
  const contextMenu = useMemo(
    () => [
      {
        id: `edit-automation-${starter.id}`,
        text: t`Edit`,
        action: () => onSelect(starter.id),
      },
      { separator: true as const },
      {
        id: `remove-automation-${starter.id}`,
        text: t`Remove`,
        action: () => removeStarterDraft.mutate(starter.id),
      },
    ],
    [onSelect, removeStarterDraft, starter.id, t],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={() => onSelect(starter.id)}
      onContextMenu={(event) => {
        onSelect(starter.id);
        void showContextMenu(event);
      }}
      className={cn([
        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        selected ? "bg-accent" : "hover:bg-accent/50",
      ])}
    >
      <span className="flex items-center gap-2">
        <span className="flex size-4 shrink-0 items-center justify-center">
          {starter.renderIcon(16)}
        </span>
        <span className="min-w-0 flex-1 truncate font-medium">
          {starter.title}
        </span>
      </span>
    </button>
  );
}

function DraftListItem({
  draftId,
  title,
  selected,
}: {
  draftId: string;
  title: string;
  selected: boolean;
}) {
  const { t } = useLingui();
  const selectDraft = useAutomationSelection((state) => state.selectDraft);
  const removeDraft = useAutomationSelection((state) => state.removeDraft);
  const contextMenu = useMemo(
    () => [
      {
        id: `edit-automation-${draftId}`,
        text: t`Edit`,
        action: () => selectDraft(draftId),
      },
      { separator: true as const },
      {
        id: `delete-automation-${draftId}`,
        text: t`Delete`,
        action: () => removeDraft(draftId),
      },
    ],
    [draftId, removeDraft, selectDraft, t],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={() => selectDraft(draftId)}
      onContextMenu={(event) => {
        selectDraft(draftId);
        void showContextMenu(event);
      }}
      className={cn([
        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        selected ? "bg-accent" : "hover:bg-accent/50",
      ])}
    >
      <span className="flex items-center gap-2">
        <Lightning className="size-4 shrink-0 text-violet-500" />
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium">{title}</span>
          <span className="text-muted-foreground mt-0.5 block truncate text-xs">
            <Trans>Draft</Trans>
          </span>
        </span>
      </span>
    </button>
  );
}

function WorkflowListItem({
  workflow,
  selected,
  onSelect,
}: {
  workflow: AutomationWorkflow;
  selected: boolean;
  onSelect: (workflowId: string, chatGroupId?: string | null) => void;
}) {
  const { t } = useLingui();
  const deleteWorkflow = useDeleteWorkflow();
  const contextMenu = useMemo(
    () => [
      {
        id: `edit-automation-${workflow.id}`,
        text: t`Edit`,
        action: () => onSelect(workflow.id, workflow.chatGroupId),
      },
      { separator: true as const },
      {
        id: `delete-automation-${workflow.id}`,
        text: t`Delete`,
        action: () => deleteWorkflow.mutate(workflow.id),
      },
    ],
    [deleteWorkflow, onSelect, t, workflow.chatGroupId, workflow.id],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);

  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={() => onSelect(workflow.id, workflow.chatGroupId)}
      onContextMenu={(event) => {
        onSelect(workflow.id, workflow.chatGroupId);
        void showContextMenu(event);
      }}
      className={cn([
        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        selected ? "bg-accent" : "hover:bg-accent/50",
      ])}
    >
      <span className="flex items-center gap-2">
        <Lightning className="size-4 shrink-0 text-violet-500" />
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium">
            {workflow.title.trim() || t`Untitled automation`}
          </span>
          <span className="text-muted-foreground mt-0.5 block truncate text-xs">
            {workflow.enabled ? <Trans>Enabled</Trans> : <Trans>Draft</Trans>}
          </span>
        </span>
      </span>
    </button>
  );
}

function ChatAutomationListItem({
  automation,
  selected,
  onSelect,
}: {
  automation: ChatGroupRecord;
  selected: boolean;
  onSelect: (groupId: string) => void;
}) {
  const { t } = useLingui();
  const deleteChatAutomation = useDeleteChatAutomation();
  const contextMenu = useMemo(
    () => [
      {
        id: `edit-automation-${automation.id}`,
        text: t`Edit`,
        action: () => onSelect(automation.id),
      },
      { separator: true as const },
      {
        id: `delete-automation-${automation.id}`,
        text: t`Delete`,
        action: () => deleteChatAutomation.mutate(automation.id),
      },
    ],
    [automation.id, deleteChatAutomation, onSelect, t],
  );
  const showContextMenu = useNativeContextMenu(contextMenu);
  const createdAt = automation.createdAt
    ? formatDistanceToNow(new Date(automation.createdAt), { addSuffix: true })
    : "";

  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={() => onSelect(automation.id)}
      onContextMenu={(event) => {
        onSelect(automation.id);
        void showContextMenu(event);
      }}
      className={cn([
        "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        selected ? "bg-accent" : "hover:bg-accent/50",
      ])}
    >
      <span className="flex items-center gap-2">
        <Lightning className="size-4 shrink-0 text-violet-500" />
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium">{automation.title}</span>
          {createdAt ? (
            <span className="text-muted-foreground mt-0.5 block truncate text-xs">
              {createdAt}
            </span>
          ) : null}
        </span>
      </span>
    </button>
  );
}
