import { useMutation } from "@tanstack/react-query";
import {
  AlertCircleIcon,
  FileIcon,
  ImageIcon,
  LoaderCircleIcon,
  PaperclipIcon,
} from "lucide-react";
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

import {
  NoteEditor,
  type NoteEditorProps,
  type NoteEditorRef,
  schema,
  setCommentAnchors,
} from "@hypr/editor/note";

import {
  collectSharedNoteComments,
  useSharedNoteComments,
} from "@/components/shared-note-comments-data";
import {
  sharedPrimaryButtonClassName,
  sharedSecondaryButtonClassName,
} from "@/components/shared-note-viewer";
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
      editAuthenticatedSharedNote({ data: input }),
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
    <div className="shared-note-web-editor">
      {conflict && (
        <div
          className="border-color-subtle bg-surface-subtle mb-5 rounded-2xl border p-4"
          role="alert"
        >
          <p className="text-color font-mono text-sm font-medium">
            This note changed elsewhere.
          </p>
          <p className="text-color-muted mt-1 text-sm leading-6">
            Reload the latest version before making more edits.
          </p>
          {confirmDiscard ? (
            <div className="mt-3">
              <p className="text-color-muted text-sm leading-6">
                Reloading will discard this draft. Copy anything you want to
                keep first.
              </p>
              <div className="mt-3 flex flex-wrap gap-3">
                <button
                  type="button"
                  className="text-color font-mono text-sm underline underline-offset-4"
                  onClick={() => setConfirmDiscard(false)}
                >
                  Keep draft
                </button>
                <button
                  type="button"
                  className="font-mono text-sm text-red-700 underline underline-offset-4"
                  onClick={() => onReloadLatest(conflict)}
                >
                  Discard draft and reload
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              className="text-color mt-3 font-mono text-sm underline underline-offset-4"
              onClick={() => setConfirmDiscard(true)}
            >
              Reload latest
            </button>
          )}
        </div>
      )}
      {(clientError || hasServerError) && (
        <div
          className="mb-5 flex gap-3 rounded-2xl border border-red-200 bg-red-50 p-4 text-red-900"
          role="alert"
        >
          <AlertCircleIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
          <p className="text-sm leading-6">
            {clientError ??
              "We couldn’t save this edit. Your draft is still here."}
          </p>
        </div>
      )}
      {availabilityIssue && (
        <div
          className="mb-5 flex gap-3 rounded-2xl border border-amber-200 bg-amber-50 p-4 text-amber-950"
          role="alert"
        >
          <AlertCircleIcon className="mt-0.5 size-4 shrink-0" aria-hidden />
          <p className="text-sm leading-6">
            {availabilityIssue === "sign_in_required"
              ? "Your session expired. Your draft is still here and can be copied before you leave the editor and sign in again."
              : "Your editing access changed. Your draft is still here and can be copied before you leave the editor."}
          </p>
        </div>
      )}

      {snapshot.attachments.length > 0 && (
        <div className="border-color-subtle bg-surface-subtle text-color-muted mb-5 flex items-start gap-3 rounded-xl border px-4 py-3 text-sm leading-6">
          <PaperclipIcon className="mt-1 size-4 shrink-0" aria-hidden />
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
          className="min-h-80 outline-hidden"
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

      <div className="border-color-subtle mt-7 flex flex-wrap justify-end gap-3 border-t pt-5">
        <button
          type="button"
          className={sharedSecondaryButtonClassName}
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
          className={sharedPrimaryButtonClassName}
          disabled={
            mutation.isPending ||
            conflict !== null ||
            availabilityIssue !== null
          }
          onClick={save}
        >
          {mutation.isPending && (
            <LoaderCircleIcon
              className="mr-2 size-4 animate-spin"
              aria-hidden
            />
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
  const Icon = isImage ? ImageIcon : FileIcon;

  return (
    <div
      ref={ref}
      {...htmlAttrs}
      contentEditable={false}
      suppressContentEditableWarning
      className="border-color-subtle bg-surface-subtle my-3 flex items-center gap-3 rounded-xl border px-4 py-3"
    >
      <div className="bg-surface flex size-10 shrink-0 items-center justify-center rounded-lg">
        <Icon className="text-color-muted size-5" aria-hidden />
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-color truncate text-sm font-medium">
          {attachment?.filename ?? "Attachment unavailable"}
        </p>
        <p className="text-color-muted mt-0.5 text-xs">
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
    <div className="border-color-subtle bg-surface-subtle rounded-2xl border p-5">
      <h2 className="text-color font-mono text-base font-medium">{title}</h2>
      <p className="text-color-muted mt-2 text-sm leading-6">{description}</p>
      <div className="mt-4">
        <button
          type="button"
          className={sharedSecondaryButtonClassName}
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
