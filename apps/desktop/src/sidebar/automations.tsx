import { Trans, useLingui } from "@lingui/react/macro";
import { Lightning, MagnifyingGlass, Plus, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { formatDistanceToNow } from "@anlg/utils";

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
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

export function AutomationsNav() {
  const { t } = useLingui();
  const [search, setSearch] = useState("");
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
    <div {...stylex.props(styles.root)}>
      <CustomSidebarHeader>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          sx={styles.newButton}
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

      <div {...stylex.props(styles.searchSection)}>
        <div {...stylex.props(styles.searchContainer)}>
          <MagnifyingGlass {...stylex.props(styles.searchIcon)} />
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
            {...stylex.props(styles.searchInput)}
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              {...stylex.props(styles.clearButton)}
              aria-label={t`Clear search`}
            >
              <X {...stylex.props(styles.clearIcon)} />
            </button>
          ) : null}
        </div>
      </div>

      <div {...stylex.props(styles.scrollArea)}>
        {isEmpty ? (
          <div {...stylex.props(styles.empty)}>
            <Lightning size={32} {...stylex.props(styles.emptyIcon)} />
            <p {...stylex.props(styles.emptyText)}>
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
              <div {...stylex.props(styles.section)}>
                <h3 {...stylex.props(styles.sectionTitle)}>
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
                <h3 {...stylex.props(styles.sectionTitle)}>
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
      {...stylex.props(
        styles.listItem,
        selected ? styles.listItemSelected : styles.listItemIdle,
      )}
    >
      <span {...stylex.props(styles.listItemRow)}>
        <span {...stylex.props(styles.listItemIconSlot)}>
          {starter.renderIcon(16)}
        </span>
        <span {...stylex.props(styles.listItemTitle)}>{starter.title}</span>
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
      {...stylex.props(
        styles.listItem,
        selected ? styles.listItemSelected : styles.listItemIdle,
      )}
    >
      <span {...stylex.props(styles.listItemRow)}>
        <Lightning {...stylex.props(styles.lightningIcon)} weight="fill" />
        <span {...stylex.props(styles.listItemText)}>
          <span {...stylex.props(styles.listItemTitle)}>{title}</span>
          <span {...stylex.props(styles.listItemMeta)}>
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
      {...stylex.props(
        styles.listItem,
        selected ? styles.listItemSelected : styles.listItemIdle,
      )}
    >
      <span {...stylex.props(styles.listItemRow)}>
        <Lightning {...stylex.props(styles.lightningIcon)} weight="fill" />
        <span {...stylex.props(styles.listItemText)}>
          <span {...stylex.props(styles.listItemTitle)}>
            {workflow.title.trim() || t`Untitled automation`}
          </span>
          <span {...stylex.props(styles.listItemMeta)}>
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
      {...stylex.props(
        styles.listItem,
        selected ? styles.listItemSelected : styles.listItemIdle,
      )}
    >
      <span {...stylex.props(styles.listItemRow)}>
        <Lightning {...stylex.props(styles.lightningIcon)} weight="fill" />
        <span {...stylex.props(styles.listItemText)}>
          <span {...stylex.props(styles.listItemTitle)}>
            {automation.title}
          </span>
          {createdAt ? (
            <span {...stylex.props(styles.listItemMeta)}>{createdAt}</span>
          ) : null}
        </span>
      </span>
    </button>
  );
}

const styles = stylex.create({
  clearButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    flexShrink: 0,
    height: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  clearIcon: {
    height: "1rem",
    width: "1rem",
  },
  empty: {
    color: colors.mutedForeground,
    paddingBlock: spacing.xxl,
    paddingInline: spacing.md,
    textAlign: "center",
  },
  emptyIcon: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    marginBottom: spacing.sm,
    marginInline: "auto",
  },
  emptyText: {
    fontSize: "0.875rem",
  },
  lightningIcon: {
    color: "rgb(139 92 246)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  listItem: {
    borderRadius: radii.lg,
    fontSize: "0.875rem",
    paddingBlock: spacing.sm,
    paddingInline: spacing.md,
    textAlign: "left",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    userSelect: "none",
    width: "100%",
  },
  listItemIconSlot: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "1rem",
    justifyContent: "center",
    width: "1rem",
  },
  listItemIdle: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.accent} 50%, transparent)`,
    },
  },
  listItemMeta: {
    color: colors.mutedForeground,
    display: "block",
    fontSize: "0.75rem",
    marginTop: "0.125rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  listItemRow: {
    alignItems: "center",
    display: "flex",
    gap: spacing.sm,
  },
  listItemSelected: {
    backgroundColor: colors.accent,
  },
  listItemText: {
    flex: "1",
    minWidth: 0,
  },
  listItemTitle: {
    display: "block",
    flex: "1",
    fontWeight: 500,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  newButton: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    position: "relative",
    zIndex: 60,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    paddingBottom: spacing.sm,
  },
  scrollArea: {
    "::-webkit-scrollbar": {
      display: "none",
    },
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    paddingTop: spacing.xs,
    scrollbarWidth: "none",
  },
  searchContainer: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.accent} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    flexShrink: 0,
    gap: spacing.sm,
    height: "2rem",
    paddingInline: spacing.md,
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  searchInput: {
    "::placeholder": {
      color: colors.mutedForeground,
      fontSize: "0.875rem",
    },
    backgroundColor: "transparent",
    flex: "1",
    fontSize: "0.875rem",
    minWidth: 0,
    outline: {
      default: null,
      ":focus": "2px solid transparent",
    },
    outlineOffset: {
      default: null,
      ":focus": "2px",
    },
  },
  searchSection: {
    paddingBottom: spacing.sm,
  },
  section: {
    paddingBottom: spacing.sm,
  },
  sectionTitle: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    paddingBottom: spacing.xs,
    paddingInline: spacing.md,
    paddingTop: spacing.xs,
  },
});
