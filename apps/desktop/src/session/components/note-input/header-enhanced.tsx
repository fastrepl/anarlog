import { CaretDown, Sparkle } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { Spinner } from "@anlg/ui/components/ui/spinner";

import {
  copyTextToClipboard,
  getIconHeaderViewStyles,
  getEnhancedNoteTitle,
  getStoredNoteMarkdown,
} from "./header-shared";
import {
  TemplatePickerPopover,
  type TemplateSelection,
} from "./template-picker";

import { useAITaskTask } from "~/ai/hooks";
import { getEnhancerService } from "~/services/enhancer";
import { useEnhancedNoteActions } from "~/session/components/note-input/enhanced-actions";
import { useEnhancedNote } from "~/session/queries";
import {
  type MenuItemDef,
  useNativeContextMenu,
} from "~/shared/hooks/useNativeContextMenu";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { useUserTemplate } from "~/templates";

export function HeaderViewEnhanced({
  isActive,
  onClick = () => {},
  sessionId,
  enhancedNoteId,
  canRemove = false,
  onRemove,
  onSelectNote,
}: {
  isActive: boolean;
  onClick?: () => void;
  sessionId: string;
  enhancedNoteId: string;
  canRemove?: boolean;
  onRemove?: () => void;
  onSelectNote?: (enhancedNoteId: string) => void;
}) {
  if (!isActive) {
    return (
      <HeaderViewEnhancedInactive
        enhancedNoteId={enhancedNoteId}
        onClick={onClick}
      />
    );
  }

  return (
    <HeaderViewEnhancedActive
      sessionId={sessionId}
      enhancedNoteId={enhancedNoteId}
      canRemove={canRemove}
      onRemove={onRemove}
      onSelectNote={onSelectNote}
    />
  );
}

function useEnhancedViewTitle(enhancedNoteId: string) {
  const enhancedNote = useEnhancedNote(enhancedNoteId);
  const rawTitle = enhancedNote?.title;
  const templateId = enhancedNote?.templateId;
  const { data: template } = useUserTemplate(templateId);
  const templateTitle = template?.title?.trim() || null;
  const viewTitle = getEnhancedNoteTitle({
    rawTitle,
    templateTitle,
    templateId,
  });

  return {
    viewTitle,
    templateTooltip:
      templateId && templateTitle
        ? `${templateTitle} was used to generate this summary.`
        : undefined,
  };
}

function useEnhancedViewGenerating(enhancedNoteId: string) {
  const taskId = createTaskId(enhancedNoteId, "enhance");
  const enhanceTask = useAITaskTask(taskId, "enhance");

  return enhanceTask.isGenerating;
}

function HeaderViewEnhancedInactive({
  onClick = () => {},
  enhancedNoteId,
}: {
  enhancedNoteId: string;
  onClick?: () => void;
}) {
  const { viewTitle, templateTooltip } = useEnhancedViewTitle(enhancedNoteId);
  const isGenerating = useEnhancedViewGenerating(enhancedNoteId);

  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={viewTitle}
      onClick={onClick}
      title={templateTooltip}
      data-header-view
      {...stylex.props(
        getIconHeaderViewStyles(false, "tray", styles.inactiveButton),
      )}
    >
      {isGenerating ? (
        <Spinner size={16} sx={styles.shrinkIcon} />
      ) : (
        <Sparkle {...stylex.props(styles.icon)} />
      )}
    </button>
  );
}

function HeaderViewEnhancedActive({
  sessionId,
  enhancedNoteId,
  canRemove = false,
  onRemove,
  onSelectNote,
}: {
  sessionId: string;
  enhancedNoteId: string;
  canRemove?: boolean;
  onRemove?: () => void;
  onSelectNote?: (enhancedNoteId: string) => void;
}) {
  const { isGenerating, isError, onRegenerate } = useEnhanceLogic(
    sessionId,
    enhancedNoteId,
  );
  const enhancedNote = useEnhancedNote(enhancedNoteId);
  const content = enhancedNote?.content;
  const usedTemplateId = enhancedNote?.templateId?.trim() || null;
  const { viewTitle, templateTooltip } = useEnhancedViewTitle(enhancedNoteId);
  const noteMarkdown = useMemo(() => getStoredNoteMarkdown(content), [content]);

  const handleCopy = useCallback(() => {
    return copyTextToClipboard(noteMarkdown, {
      success: `${viewTitle} copied to clipboard`,
      error: `Failed to copy ${viewTitle}`,
    });
  }, [noteMarkdown, viewTitle]);
  const handleRegenerate = useCallback(() => {
    void onRegenerate(null);
  }, [onRegenerate]);
  const handleSelectTemplate = useCallback(
    (selection: TemplateSelection) => {
      if (isGenerating) {
        return;
      }

      const service = getEnhancerService();
      if (!service) {
        return;
      }

      onSelectNote?.(enhancedNoteId);

      void Promise.resolve(
        service.enhance(sessionId, {
          templateId: selection.templateId,
          targetNoteId: enhancedNoteId,
          templateTitle: selection.templateId ? selection.title : undefined,
        }),
      )
        .then((result) => {
          if (
            (result.type === "started" || result.type === "already_active") &&
            result.noteId !== enhancedNoteId
          ) {
            onSelectNote?.(result.noteId);
          }
        })
        .catch((error) => {
          console.error("[enhancer] failed to replace summary template", error);
        });
    },
    [enhancedNoteId, isGenerating, onSelectNote, sessionId],
  );
  const contextMenu = useMemo<MenuItemDef[]>(() => {
    const items: MenuItemDef[] = [
      {
        id: `copy-enhanced-${enhancedNoteId}`,
        text: "Copy",
        action: () => {
          void handleCopy();
        },
        disabled: noteMarkdown.length === 0,
      },
      {
        id: `regenerate-enhanced-${enhancedNoteId}`,
        text: "Regenerate",
        action: handleRegenerate,
        disabled: isGenerating,
      },
    ];

    if (canRemove) {
      items.push({ separator: true });
      items.push({
        id: `remove-enhanced-${enhancedNoteId}`,
        text: "Remove",
        action: () => {
          onRemove?.();
        },
        disabled: isGenerating || !onRemove,
      });
    }

    return items;
  }, [
    canRemove,
    enhancedNoteId,
    handleCopy,
    handleRegenerate,
    isGenerating,
    noteMarkdown.length,
    onRemove,
  ]);
  const showContextMenu = useNativeContextMenu(contextMenu);
  const templateMenuTrigger = (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={viewTitle}
      aria-current="page"
      aria-disabled={isGenerating}
      tabIndex={isGenerating ? -1 : 0}
      onClick={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={showContextMenu}
      title={templateTooltip}
      data-header-view
      {...stylex.props(
        getIconHeaderViewStyles(true, "tray", [
          styles.activeButton,
          isGenerating ? styles.generating : styles.interactive,
          isError ? styles.error : styles.normal,
        ]),
      )}
    >
      {isGenerating ? (
        <Spinner size={16} sx={styles.shrinkIcon} />
      ) : (
        <Sparkle {...stylex.props(styles.icon)} />
      )}
      <span {...stylex.props(styles.label)}>{viewTitle}</span>
      <CaretDown {...stylex.props(styles.caret)} />
    </button>
  );

  return (
    <TemplatePickerPopover
      onSelectTemplate={handleSelectTemplate}
      usedTemplateId={usedTemplateId}
      onRegenerateUsed={handleRegenerate}
      isRegenerating={isGenerating}
      trigger={templateMenuTrigger}
    />
  );
}

const useEnhanceLogic = (sessionId: string, enhancedNoteId: string) =>
  useEnhancedNoteActions({ sessionId, enhancedNoteId });

const compact = "@container (max-width: 480px)";

const styles = stylex.create({
  activeButton: {
    gap: {
      default: "0.375rem",
      [compact]: 0,
    },
    maxWidth: {
      default: "14rem",
      [compact]: "3rem",
    },
    minWidth: {
      default: "62px",
      [compact]: "3rem",
    },
    paddingInline: {
      default: "0.5rem",
      [compact]: "0.375rem",
    },
  },
  caret: {
    height: "0.875rem",
    width: "0.875rem",
  },
  error: {
    backgroundColor: {
      default: null,
      ":hover": "rgb(254 242 242)",
      ":focus-visible": "rgb(254 242 242)",
      ":is(.dark *):hover": "rgb(69 10 10 / 0.5)",
      ":is(.dark *):focus-visible": "rgb(69 10 10 / 0.5)",
    },
    color: {
      default: "rgb(220 38 38)",
      ":hover": "rgb(185 28 28)",
      ":is(.dark *)": "rgb(248 113 113)",
      ":is(.dark *):hover": "rgb(252 165 165)",
    },
  },
  generating: {
    cursor: "not-allowed",
    opacity: 0.7,
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  inactiveButton: {
    paddingInline: "0.5rem",
  },
  interactive: {
    cursor: "pointer",
  },
  label: {
    clip: {
      default: null,
      [compact]: "rect(0, 0, 0, 0)",
    },
    fontSize: "0.75rem",
    fontWeight: 500,
    height: {
      default: null,
      [compact]: "1px",
    },
    margin: {
      default: null,
      [compact]: "-1px",
    },
    minWidth: 0,
    overflow: "hidden",
    padding: {
      default: null,
      [compact]: 0,
    },
    position: {
      default: null,
      [compact]: "absolute",
    },
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    width: {
      default: null,
      [compact]: "1px",
    },
  },
  normal: {
    backgroundColor: {
      default: null,
      ":focus-visible": "white",
    },
    color: {
      default: null,
      ":focus-visible": colors.foreground,
      ":is(.dark *):focus-visible": colors.primary,
    },
  },
  shrinkIcon: {
    flexShrink: 0,
  },
});
