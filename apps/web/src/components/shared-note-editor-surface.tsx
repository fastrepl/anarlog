import {
  CircleNotch,
  File,
  Image,
  Paperclip,
  WarningCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import {
  type ComponentProps,
  createContext,
  forwardRef,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { colors, fonts } from "@anlg/design-system/tokens.stylex";
import {
  NoteEditor,
  type NoteEditorProps,
  type NoteEditorRef,
  schema,
  setCommentAnchors,
} from "@anlg/editor/note";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import {
  collectSharedNoteComments,
  useSharedNoteComments,
} from "@/components/shared-note-comments-data";
import { sharedButtonStyles } from "@/components/shared-note-viewer";
import { editAuthenticatedSharedNote } from "@/functions/shared-notes";
import { resolveSharedNoteCommentRanges } from "@/lib/shared-note-comment-anchors";
import {
  buildSharedNoteWebEditInput,
  canonicalizeSharedNoteWebDraft,
  ensureSharedNoteEditorTitle,
  hasUnsupportedSharedNoteEditorNode,
  reuseSharedNoteMutationIdForUnchangedDraft,
  type SharedNoteWebEditInput,
} from "@/lib/shared-note-editing";
import type {
  SharedNoteAttachment,
  SharedNoteDocument,
  SharedNoteNode,
  SharedNoteSnapshot,
  SharedNoteWebEditSnapshot,
} from "@/lib/shared-notes";

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style2: {
    marginBottom: "1.25rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1rem",
  },
  style3: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
  },
  style4: {
    marginTop: ".25rem",
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style5: {
    marginTop: ".75rem",
  },
  style6: {
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style7: {
    marginTop: ".75rem",
    display: "flex",
    flexWrap: "wrap",
    gap: ".75rem",
  },
  style8: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    textDecorationLine: "underline",
    textUnderlineOffset: "4px",
  },
  style9: {
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
    textDecorationLine: "underline",
    textUnderlineOffset: "4px",
  },
  style10: {
    marginTop: ".75rem",
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    textDecorationLine: "underline",
    textUnderlineOffset: "4px",
  },
  style11: {
    marginBottom: "1.25rem",
    display: "flex",
    gap: ".75rem",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#fecaca",
    backgroundColor: "#fef2f2",
    padding: "1rem",
    color: "#7f1d1d",
  },
  style12: {
    marginTop: ".125rem",
    width: "1rem",
    height: "1rem",
    flexShrink: 0,
  },
  style13: {
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style14: {
    marginBottom: "1.25rem",
    display: "flex",
    gap: ".75rem",
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: "#fde68a",
    backgroundColor: "#fffbeb",
    padding: "1rem",
    color: "#451a03",
  },
  style15: {
    marginBottom: "1.25rem",
    display: "flex",
    alignItems: "flex-start",
    gap: ".75rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style16: {
    marginTop: ".25rem",
    width: "1rem",
    height: "1rem",
    flexShrink: 0,
  },
  style17: {
    minHeight: "20rem",
    outlineStyle: "none",
    outlineOffset: {
      default: null,
      "@media (forced-colors: active)": "2px",
    },
    outline: {
      default: null,
      "@media (forced-colors: active)": "2px solid #0000",
    },
  },
  style18: {
    marginTop: "1.75rem",
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: ".75rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1.25rem",
  },
  style19: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: spin,
  },
  style20: {
    marginBlock: ".75rem",
    display: "flex",
    alignItems: "center",
    gap: ".75rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
  },
  style21: {
    display: "flex",
    width: "2.5rem",
    height: "2.5rem",
    flexShrink: 0,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: colors.card,
    borderRadius: ".5rem",
  },
  style22: {
    color: colors.mutedForeground,
    width: "1.25rem",
    height: "1.25rem",
  },
  style23: {
    minWidth: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
  },
  style24: {
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: colors.foreground,
  },
  style25: {
    marginTop: ".125rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  style26: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    padding: "1.25rem",
  },
  style27: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
  },
  style28: {
    marginTop: ".5rem",
    color: colors.mutedForeground,
    fontSize: ".875rem",
    lineHeight: "1.5rem",
  },
  style29: {
    marginTop: "1rem",
  },
});
type EditorNodeView = NonNullable<NoteEditorProps["extraNodeViews"]>[string];
type EditorNodeViewProps = ComponentProps<EditorNodeView>;
const SharedEditorAttachmentsContext = createContext<
  ReadonlyMap<string, SharedNoteAttachment>
>(new Map());
export function SharedNoteEditorSurface({
  onCancel,
  onReloadLatest,
  onSaved,
  onUnavailable,
  snapshot,
}: {
  onCancel: () => void;
  onReloadLatest: (edited: SharedNoteWebEditSnapshot) => void;
  onSaved: (edited: SharedNoteWebEditSnapshot) => void;
  onUnavailable: (reason: "access_changed" | "sign_in_required") => void;
  snapshot: SharedNoteSnapshot;
}) {
  const editorRef = useRef<NoteEditorRef>(null);
  const [editorView, setEditorView] = useState<NonNullable<
    NoteEditorRef["view"]
  > | null>(null);
  const [clientError, setClientError] = useState<string | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const commentsQuery = useSharedNoteComments({
    shareId: snapshot.shareId,
    enabled: true,
  });
  const comments = useMemo(
    () => collectSharedNoteComments(commentsQuery.data),
    [commentsQuery.data],
  );
  // External sync: resolved anchor highlights are pushed into the ProseMirror
  // plugin; decoration mapping then tracks live edits on its own.
  useEffect(() => {
    if (!editorView) return;
    const anchored = resolveSharedNoteCommentRanges(
      editorView.state.doc,
      comments,
      snapshot.contentRevision,
    );
    setCommentAnchors(
      editorView,
      anchored.flatMap((comment) =>
        comment.range
          ? [
              {
                commentId: comment.commentId,
                from: comment.range.from,
                to: comment.range.to,
              },
            ]
          : [],
      ),
    );
  }, [editorView, comments, snapshot.contentRevision]);
  const attachmentById = useMemo(
    () =>
      new Map(
        snapshot.attachments.map((attachment) => [attachment.id, attachment]),
      ),
    [snapshot.attachments],
  );
  const { initialContent, initialContentIsValid } = useMemo(() => {
    const content = ensureSharedNoteEditorTitle(snapshot.body, snapshot.title);
    return {
      initialContent: content,
      initialContentIsValid:
        !hasUnsupportedSharedNoteEditorNode(content) &&
        isCanonicalEditorDocument(content),
    };
  }, [snapshot]);
  const mutation = useMutation({
    mutationFn: (input: SharedNoteWebEditInput) =>
      editAuthenticatedSharedNote({
        data: input,
      }),
    onSuccess: (result) => {
      if (result.status === "ready") onSaved(result);
    },
  });
  if (!initialContentIsValid) {
    return (
      <EditorMessage
        description="This note uses content the web editor can’t safely preserve yet. You can still view it here or edit it in the Anarlog app."
        onCancel={onCancel}
        title="This note isn’t ready for web editing"
      />
    );
  }
  const conflict = mutation.data?.status === "conflict" ? mutation.data : null;
  const hasServerError = mutation.isError || mutation.data?.status === "error";
  const availabilityIssue =
    mutation.data?.status === "sign_in_required"
      ? "sign_in_required"
      : mutation.data?.status === "unavailable"
        ? "access_changed"
        : null;
  const save = () => {
    const view = editorRef.current?.view;
    if (!view) return;
    const body = view.state.doc.toJSON() as SharedNoteDocument;
    const canonicalBody = canonicalizeSharedNoteWebDraft(
      body,
      snapshot.attachments.map(({ id }) => id),
    );
    if (
      hasUnsupportedSharedNoteEditorNode(body) ||
      !canonicalBody ||
      !isCanonicalEditorDocument(canonicalBody)
    ) {
      setClientError(
        "This edit includes content the web editor can’t safely save yet.",
      );
      return;
    }
    setClientError(null);
    const input = buildSharedNoteWebEditInput({
      body: canonicalBody,
      mutationId: crypto.randomUUID(),
      snapshot,
    });
    mutation.mutate(
      reuseSharedNoteMutationIdForUnchangedDraft(
        input,
        hasServerError ? mutation.variables : undefined,
      ),
    );
  };
  return (
    <div>
      {conflict && (
        <div {...stylex.props(styles.style2)} role="alert">
          <p {...stylex.props(styles.style3)}>This note changed elsewhere.</p>
          <p {...stylex.props(styles.style4)}>
            Reload the latest version before making more edits.
          </p>
          {confirmDiscard ? (
            <div {...stylex.props(styles.style5)}>
              <p {...stylex.props(styles.style6)}>
                Reloading will discard this draft. Copy anything you want to
                keep first.
              </p>
              <div {...stylex.props(styles.style7)}>
                <button
                  type="button"
                  {...stylex.props(styles.style8)}
                  onClick={() => setConfirmDiscard(false)}
                >
                  Keep draft
                </button>
                <button
                  type="button"
                  {...stylex.props(styles.style9)}
                  onClick={() => onReloadLatest(conflict)}
                >
                  Discard draft and reload
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              {...stylex.props(styles.style10)}
              onClick={() => setConfirmDiscard(true)}
            >
              Reload latest
            </button>
          )}
        </div>
      )}
      {(clientError || hasServerError) && (
        <div {...stylex.props(styles.style11)} role="alert">
          <WarningCircle {...stylex.props(styles.style12)} aria-hidden />
          <p {...stylex.props(styles.style13)}>
            {clientError ??
              "We couldn’t save this edit. Your draft is still here."}
          </p>
        </div>
      )}
      {availabilityIssue && (
        <div {...stylex.props(styles.style14)} role="alert">
          <WarningCircle {...stylex.props(styles.style12)} aria-hidden />
          <p {...stylex.props(styles.style13)}>
            {availabilityIssue === "sign_in_required"
              ? "Your session expired. Your draft is still here and can be copied before you leave the editor and sign in again."
              : "Your editing access changed. Your draft is still here and can be copied before you leave the editor."}
          </p>
        </div>
      )}

      {snapshot.attachments.length > 0 && (
        <div {...stylex.props(styles.style15)}>
          <Paperclip {...stylex.props(styles.style16)} aria-hidden />
          <span>
            {snapshot.attachments.length}{" "}
            {snapshot.attachments.length === 1 ? "attachment" : "attachments"}
            {" will stay included with this note."}
          </span>
        </div>
      )}

      <SharedEditorAttachmentsContext.Provider value={attachmentById}>
        <NoteEditor
          ref={editorRef}
          {...mergeStyleXProps(styles.style17, "session-note-editor")}
          commentAnchorsEnabled
          extraNodeViews={lockedAttachmentNodeViews}
          initialContent={initialContent}
          handleChange={() => {
            setClientError(null);
            if (hasServerError && !mutation.isPending) mutation.reset();
          }}
          onViewReady={setEditorView}
          onViewDisposed={() => setEditorView(null)}
          readOnly={
            mutation.isPending ||
            conflict !== null ||
            availabilityIssue !== null
          }
          showFormatToolbar={false}
          showSlashCommand={false}
        />
      </SharedEditorAttachmentsContext.Provider>

      <div {...stylex.props(styles.style18)}>
        <button
          type="button"
          {...stylex.props([
            sharedButtonStyles.base,
            sharedButtonStyles.secondary,
          ])}
          disabled={mutation.isPending}
          onClick={() => {
            if (availabilityIssue) {
              onUnavailable(availabilityIssue);
            } else {
              onCancel();
            }
          }}
        >
          {availabilityIssue ? "Leave editor" : "Cancel"}
        </button>
        <button
          type="button"
          {...stylex.props([
            sharedButtonStyles.base,
            sharedButtonStyles.primary,
          ])}
          disabled={
            mutation.isPending ||
            conflict !== null ||
            availabilityIssue !== null
          }
          onClick={save}
        >
          {mutation.isPending && (
            <CircleNotch {...stylex.props(styles.style19)} aria-hidden />
          )}
          {mutation.isPending
            ? "Saving…"
            : hasServerError
              ? "Try again"
              : "Save"}
        </button>
      </div>
    </div>
  );
}
const LockedSharedAttachmentView = forwardRef<
  HTMLDivElement,
  EditorNodeViewProps
>(function LockedSharedAttachmentView({ nodeProps, ...htmlAttrs }, ref) {
  const attachments = useContext(SharedEditorAttachmentsContext);
  const sharedAttachmentId = nodeProps.node.attrs.sharedAttachmentId;
  const attachment =
    typeof sharedAttachmentId === "string"
      ? attachments.get(sharedAttachmentId)
      : undefined;
  const isImage = nodeProps.node.type.name === "image";
  const Icon = isImage ? Image : File;
  return (
    <div
      ref={ref}
      {...htmlAttrs}
      contentEditable={false}
      suppressContentEditableWarning
      {...stylex.props(styles.style20)}
    >
      <div {...stylex.props(styles.style21)}>
        <Icon {...stylex.props(styles.style22)} aria-hidden />
      </div>
      <div {...stylex.props(styles.style23)}>
        <p {...stylex.props(styles.style24)}>
          {attachment?.filename ?? "Attachment unavailable"}
        </p>
        <p {...stylex.props(styles.style25)}>
          {attachment
            ? `${formatFileSize(attachment.sizeBytes)} · Included with shared note`
            : "Included attachment"}
        </p>
      </div>
    </div>
  );
});
const lockedAttachmentNodeViews = {
  fileAttachment: LockedSharedAttachmentView,
  image: LockedSharedAttachmentView,
};
function EditorMessage({
  description,
  onCancel,
  title,
}: {
  description: string;
  onCancel: () => void;
  title: string;
}) {
  return (
    <div {...stylex.props(styles.style26)}>
      <h2 {...stylex.props(styles.style27)}>{title}</h2>
      <p {...stylex.props(styles.style28)}>{description}</p>
      <div {...stylex.props(styles.style29)}>
        <button
          type="button"
          {...stylex.props([
            sharedButtonStyles.base,
            sharedButtonStyles.secondary,
          ])}
          onClick={onCancel}
        >
          Back to note
        </button>
      </div>
    </div>
  );
}
function isCanonicalEditorDocument(document: SharedNoteDocument) {
  const stack: SharedNoteNode[] = [document];
  while (stack.length > 0) {
    const node = stack.pop();
    if (!node) continue;
    const nodeType = schema.nodes[node.type];
    if (
      !nodeType ||
      hasUnknownAttributes(node.attrs, nodeType.spec.attrs ?? {})
    ) {
      return false;
    }
    for (const mark of node.marks ?? []) {
      const markType = schema.marks[mark.type];
      if (
        !markType ||
        hasUnknownAttributes(mark.attrs, markType.spec.attrs ?? {})
      ) {
        return false;
      }
    }
    if (node.content) stack.push(...node.content);
  }
  try {
    schema.nodeFromJSON(document).check();
    return true;
  } catch {
    return false;
  }
}
function hasUnknownAttributes(
  attrs: Record<string, unknown> | undefined,
  allowed: Record<string, unknown>,
) {
  return Object.keys(attrs ?? {}).some((key) => !(key in allowed));
}
function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
