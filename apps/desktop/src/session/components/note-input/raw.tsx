import { useLingui } from "@lingui/react/macro";
import type { EditorView } from "prosemirror-view";
import { forwardRef, useCallback, useMemo, useRef, useState } from "react";

import { parseJsonContent } from "@anlg/editor/markdown";
import {
  NoteEditor,
  type JSONContent,
  type NoteEditorRef,
  normalizePortableAttachmentUrls,
} from "@anlg/editor/note";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { Plus } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { AudioDropTarget } from "./audio-drop-target";
import { CreateBriefSuggestion } from "./create-brief-suggestion";
import { useNoteFileHandlerConfig } from "./file-handler";
import { MeetingChatHighlights } from "./meeting-chat-highlights";

import { trackAnalyticsEvent } from "~/analytics";
import { useAudioPlayer } from "~/audio-player";
import { useSessionEventParticipants } from "~/calendar/queries";
import { AppLinkView } from "~/editor-bridge/app-link-view";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { openEditorLink } from "~/editor-bridge/open-editor-link";
import { sessionMentionDropConfig } from "~/editor-bridge/session-mention-drop";
import { SessionNodeView } from "~/editor-bridge/session-view";
import { useSessionCommentAnchors } from "~/session-sharing/comment-anchors";
import { useCanShowTranscript } from "~/session/components/shared";
import { useAttachmentResolver } from "~/session/hooks/useAttachmentResolver";
import { useCreatePreMeetingBrief } from "~/session/hooks/useCreatePreMeetingBrief";
import { useUpdateSession } from "~/session/queries";
import { removeDocumentTitle } from "~/session/title-content";
import { useListener } from "~/stt/contexts";
import {
  TemplateIconGlyph,
  type UserTemplate,
  useCreateTemplate,
  useOpenTemplatesTab,
  useUserTemplates,
} from "~/templates";

const extraNodeViews = { appLink: AppLinkView, session: SessionNodeView };
const noop = () => {};
const defaultSuggestedTemplateIds = [
  "default-project-kickoff",
  "default-daily-standup",
  "default-one-on-one-meeting",
];
// Structural check: any block beyond a single empty paragraph (checkbox, list,
// extra empty lines) counts as content, unlike the text-based hasStoredNoteContent
export function isPristineNoteDoc(doc: JSONContent): boolean {
  const content = doc.content ?? [];
  if (content.length === 0) return true;
  if (content.length > 1) return false;
  const node = content[0];
  return node.type === "paragraph" && !node.content?.length;
}

const contextualTemplateRules = [
  {
    id: "default-sales-discovery-call",
    pattern:
      /\b(sales|discovery|demo|prospect|lead|client|customer|pipeline|qualification|qualifying|pitch)\b/i,
  },
  {
    id: "default-project-kickoff",
    pattern: /\b(kickoff|kick-off|project launch)\b/i,
  },
  {
    id: "default-daily-standup",
    pattern: /\b(standup|stand-up|daily sync|scrum)\b/i,
  },
  {
    id: "default-one-on-one-meeting",
    pattern: /\b(1:1|one[- ]on[- ]one)\b/i,
  },
];

export const RawEditor = forwardRef<
  NoteEditorRef,
  {
    sessionId: string;
    rawMd: string;
    sessionTitle: string;
    eventTitle?: string;
    eventDescription?: string;
    className?: string;
    onNavigateToTitle?: (pixelWidth?: number) => void;
    syncTasks?: boolean;
    showFormatToolbar?: boolean;
    onViewReady?: (view: EditorView) => void;
    onViewDisposed?: (view: EditorView) => void;
  }
>(
  (
    {
      sessionId,
      rawMd,
      sessionTitle,
      eventTitle,
      eventDescription,
      className,
      onNavigateToTitle,
      syncTasks = true,
      showFormatToolbar = true,
      onViewReady,
      onViewDisposed,
    },
    ref,
  ) => {
    const { t } = useLingui();
    const updateSession = useUpdateSession(sessionId);
    const { audioExists, audioExistsResolved } = useAudioPlayer();
    const canShowTranscript = useCanShowTranscript(sessionId, { audioExists });
    const resolveAttachment = useAttachmentResolver(sessionId);
    const { audioDropTargetProps, fileHandlerConfig, isAudioDragActive } =
      useNoteFileHandlerConfig(sessionId);
    const initialContent = useMemo<JSONContent>(
      () => removeDocumentTitle(parseJsonContent(rawMd), sessionTitle),
      [rawMd, sessionTitle],
    );
    const persistedIsMemoEmpty = useMemo(
      () => isPristineNoteDoc(initialContent),
      [initialContent],
    );
    const [liveMemoState, setLiveMemoState] = useState(() => ({
      sessionId,
      isEmpty: persistedIsMemoEmpty,
    }));
    const isMemoEmpty =
      liveMemoState.sessionId === sessionId
        ? liveMemoState.isEmpty
        : persistedIsMemoEmpty;
    const editorRef = useRef<NoteEditorRef>(null);
    const setEditorRef = useCallback(
      (editor: NoteEditorRef | null) => {
        editorRef.current = editor;
        if (typeof ref === "function") {
          ref(editor);
        } else if (ref) {
          ref.current = editor;
        }
      },
      [ref],
    );

    const persistChange = useCallback(
      (input: JSONContent, templateId?: string) => {
        const portableInput = normalizePortableAttachmentUrls(input);
        return updateSession({
          raw_md: JSON.stringify(portableInput),
          ...(templateId ? { raw_template_id: templateId } : {}),
        });
      },
      [updateSession],
    );

    const pendingTemplateIdRef = useRef<string | undefined>(undefined);
    const hasTrackedWriteRef = useRef(false);
    const trackedSessionIdRef = useRef(sessionId);
    if (trackedSessionIdRef.current !== sessionId) {
      trackedSessionIdRef.current = sessionId;
      hasTrackedWriteRef.current = false;
    }

    const hasNonEmptyText = useCallback(
      (node?: JSONContent): boolean =>
        !!node?.text?.trim() ||
        !!node?.content?.some((child: JSONContent) => hasNonEmptyText(child)),
      [],
    );

    const handleChange = useCallback(
      (input: JSONContent) => {
        const templateId = pendingTemplateIdRef.current;
        pendingTemplateIdRef.current = undefined;
        void persistChange(input, templateId).catch((error) => {
          console.error("[raw-editor] failed to persist note", error);
        });

        if (!hasTrackedWriteRef.current) {
          const hasContent = hasNonEmptyText(input);
          if (hasContent) {
            hasTrackedWriteRef.current = true;
            void trackNoteEdited();
          }
        }
      },
      [persistChange, hasNonEmptyText],
    );

    const handleDocumentChange = useCallback(
      (input: JSONContent) => {
        const isEmpty = isPristineNoteDoc(input);
        setLiveMemoState((current) => {
          if (current.sessionId === sessionId && current.isEmpty === isEmpty) {
            return current;
          }
          return { sessionId, isEmpty };
        });
      },
      [sessionId],
    );

    const getMemoBriefEditor = useCallback(() => {
      const editor = editorRef.current;
      if (!editor) {
        return null;
      }
      return {
        replaceContent: (content: JSONContent) => {
          editor.commands.replaceContent(content);
        },
        flushPendingChanges: () => {
          editor.flushPendingChanges();
        },
      };
    }, []);
    const sessionMode = useListener((state) => state.getSessionMode(sessionId));
    const {
      visible: briefVisible,
      isGenerating,
      createBrief,
    } = useCreatePreMeetingBrief({
      sessionId,
      sessionMode,
      isMemoView: true,
      isMemoEmpty,
      onSwitchToMemos: noop,
      getMemoEditor: getMemoBriefEditor,
    });

    const handleApplyTemplate = useCallback((template: UserTemplate) => {
      const content = template.sections.flatMap((section) => {
        const title = section.title.trim();
        if (!title) {
          return [];
        }

        return [
          {
            type: "heading",
            attrs: { level: 2 },
            content: [{ type: "text", text: title }],
          },
          { type: "paragraph" },
        ];
      });
      if (content.length === 0) {
        return;
      }

      const editor = editorRef.current;
      if (!editor) return;

      const nextContent = { type: "doc", content };
      pendingTemplateIdRef.current = template.id;
      editor.commands.replaceContent(nextContent);
      editor.flushPendingChanges();
      trackAnalyticsEvent("template_applied", {
        entry_point: "memo",
      });
    }, []);

    const mentionConfig = useMentionConfig();
    const commentAnchors = useSessionCommentAnchors(sessionId);
    // Stable identity: NoteEditor keys whole-document memos off this.
    const taskSource = useMemo(
      () =>
        syncTasks
          ? ({ type: "session_raw_note", id: sessionId } as const)
          : undefined,
      [syncTasks, sessionId],
    );
    const placeholderComponent = useMemo(
      () => () => (isGenerating ? t`Creating brief...` : t`Start writing...`),
      [isGenerating, t],
    );
    return (
      <AudioDropTarget
        targetProps={audioDropTargetProps}
        isActive={isAudioDragActive}
      >
        <>
          <div className="relative min-h-full">
            <NoteEditor
              ref={setEditorRef}
              className={cn(["session-note-editor", className])}
              key={`session-${sessionId}-raw`}
              initialContent={initialContent}
              resolveAttachment={resolveAttachment}
              handleChange={handleChange}
              onDocumentChange={handleDocumentChange}
              placeholderComponent={placeholderComponent}
              readOnly={isGenerating}
              mentionConfig={mentionConfig}
              sessionMentionDropConfig={sessionMentionDropConfig}
              onNavigateToTitle={onNavigateToTitle}
              onLinkOpen={openEditorLink}
              fileHandlerConfig={fileHandlerConfig}
              taskSource={taskSource}
              extraNodeViews={extraNodeViews}
              showFormatToolbar={showFormatToolbar}
              enforceTitleHeading={false}
              commentAnchorsEnabled
              onViewReady={(view) => {
                commentAnchors.onViewReady(view);
                onViewReady?.(view);
              }}
              onViewDisposed={(view) => {
                commentAnchors.onViewDisposed(view);
                onViewDisposed?.(view);
              }}
            />
            {isMemoEmpty && !isGenerating ? (
              <div className="pointer-events-none absolute inset-x-0 top-8 z-10 flex flex-col">
                {briefVisible ? (
                  <CreateBriefSuggestion onCreate={createBrief} />
                ) : null}
                {audioExistsResolved && !canShowTranscript ? (
                  <TemplateEmptyState
                    sessionId={sessionId}
                    eventTitle={eventTitle ?? sessionTitle}
                    eventDescription={eventDescription}
                    onApply={handleApplyTemplate}
                  />
                ) : null}
              </div>
            ) : null}
          </div>
          <MeetingChatHighlights sessionId={sessionId} />
        </>
      </AudioDropTarget>
    );
  },
);

function getFavoriteTemplates(templates: UserTemplate[]) {
  return [...templates]
    .filter(
      (template) =>
        template.pinned &&
        template.sections.some((section) => section.title.trim()),
    )
    .sort((a, b) => {
      const orderA = a.pinOrder ?? Infinity;
      const orderB = b.pinOrder ?? Infinity;
      if (orderA !== orderB) {
        return orderA - orderB;
      }
      return (a.title || "").localeCompare(b.title || "");
    });
}

function getSuggestedTemplates(
  templates: UserTemplate[],
  favoriteTemplates: UserTemplate[],
  {
    eventTitle,
    eventDescription,
    participantCount,
  }: {
    eventTitle?: string;
    eventDescription?: string;
    participantCount: number;
  },
) {
  const favoriteIds = new Set(favoriteTemplates.map((template) => template.id));
  const availableTemplates = templates.filter(
    (template) =>
      !favoriteIds.has(template.id) &&
      template.sections.some((section) => section.title.trim()),
  );
  const eventText = `${eventTitle ?? ""} ${eventDescription ?? ""}`;
  const contextualIds = contextualTemplateRules
    .filter((rule) => rule.pattern.test(eventText))
    .map((rule) => rule.id);
  if (participantCount === 2) {
    contextualIds.push("default-one-on-one-meeting");
  }
  const preferredTemplateIds = [
    ...new Set([...contextualIds, ...defaultSuggestedTemplateIds]),
  ];
  const preferredTemplates = preferredTemplateIds.flatMap((id) => {
    const template = availableTemplates.find(
      (candidate) => candidate.id === id,
    );
    return template ? [template] : [];
  });
  const preferredIds = new Set(
    preferredTemplates.map((template) => template.id),
  );
  const fallbackTemplates = availableTemplates
    .filter((template) => !preferredIds.has(template.id))
    .sort((a, b) => (a.title || "").localeCompare(b.title || ""));

  return [...preferredTemplates, ...fallbackTemplates].slice(
    0,
    defaultSuggestedTemplateIds.length,
  );
}

function TemplateEmptyState({
  sessionId,
  eventTitle,
  eventDescription,
  onApply,
}: {
  sessionId: string;
  eventTitle?: string;
  eventDescription?: string;
  onApply: (template: UserTemplate) => void;
}) {
  const { t } = useLingui();
  const userTemplates = useUserTemplates();
  const eventParticipants = useSessionEventParticipants(sessionId);
  const createTemplate = useCreateTemplate("session_note");
  const openTemplatesTab = useOpenTemplatesTab();
  const favoriteTemplates = useMemo(
    () => getFavoriteTemplates(userTemplates),
    [userTemplates],
  );
  const suggestedTemplates = useMemo(
    () =>
      getSuggestedTemplates(userTemplates, favoriteTemplates, {
        eventTitle,
        eventDescription,
        participantCount: eventParticipants.length,
      }),
    [
      eventDescription,
      eventParticipants.length,
      eventTitle,
      favoriteTemplates,
      userTemplates,
    ],
  );
  const handleCreateTemplate = useCallback(() => {
    void (async () => {
      const templateId = await createTemplate({
        title: "New Template",
        description: "",
        sections: [],
      });
      if (!templateId) {
        return;
      }

      openTemplatesTab({
        selectedMineId: templateId,
        selectedWebIndex: null,
        isWebMode: false,
        showHomepage: false,
      });
    })();
  }, [createTemplate, openTemplatesTab]);

  return (
    <>
      <TemplateSection
        label={t`Start with a favorite template`}
        templates={favoriteTemplates}
        onApply={onApply}
      />
      <TemplateSection
        label={t`Suggested templates`}
        templates={suggestedTemplates}
        onApply={onApply}
      />
      <button
        type="button"
        onClick={handleCreateTemplate}
        className={cn([
          "hover:bg-accent focus-visible:bg-accent pointer-events-auto -ml-2 flex h-8 w-fit max-w-full items-center gap-2 rounded-md px-2 text-left",
          "text-muted-foreground hover:text-foreground focus-visible:text-foreground transition-colors focus-visible:outline-hidden",
        ])}
      >
        <Plus aria-hidden className="size-4" />
        <span className="text-sm font-medium">{t`New template`}</span>
      </button>
    </>
  );
}

function TemplateSection({
  label,
  templates,
  onApply,
}: {
  label: string;
  templates: UserTemplate[];
  onApply: (template: UserTemplate) => void;
}) {
  const { t } = useLingui();
  if (templates.length === 0) {
    return null;
  }

  return (
    <>
      <p className="text-muted-foreground flex h-8 items-center text-xs">
        {label}
      </p>
      {templates.map((template) => (
        <button
          key={template.id}
          type="button"
          onClick={() => onApply(template)}
          className={cn([
            "hover:bg-accent focus-visible:bg-accent pointer-events-auto -ml-2 flex h-8 w-fit max-w-full items-center gap-2 rounded-md px-2 text-left",
            "text-muted-foreground hover:text-foreground focus-visible:text-foreground transition-colors focus-visible:outline-hidden",
          ])}
        >
          <TemplateIconGlyph icon={template.icon} className="size-4 text-sm" />
          <span className="min-w-0 truncate text-sm font-medium">
            {template.title || t`Untitled`}
          </span>
        </button>
      ))}
    </>
  );
}

async function trackNoteEdited() {
  try {
    await analyticsCommands.event({
      event: "note_edited",
      has_content: true,
    });
  } catch (error) {
    console.error("[raw-editor] failed to record note analytics", error);
  }
}
