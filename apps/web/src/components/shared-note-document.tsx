import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import {
  Fragment,
  createContext,
  createElement,
  type ReactNode,
  useContext,
  useMemo,
  useRef,
  useState,
} from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";
// The read surface that otherwise carries these styles is lazy-loaded, so the
// static document would render unstyled during SSR and on the fallback paths.
import "@anlg/editor/styles.css";

import {
  getSafeSharedNoteHref,
  isMatchingSharedNoteAttachmentDownload,
  type SharedNoteAttachment,
  type SharedNoteAttachmentDownload,
  type SharedNoteNode,
} from "@/lib/shared-notes";
const styles = stylex.create({
  style1: {
    color: colors.foreground,
  },
  style2: {
    marginTop: "2.5rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingTop: "1.5rem",
  },
  style3: {
    marginBlock: "2rem",
    borderColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
  },
  style4: {
    listStyleType: "disc",
    paddingLeft: "1.5rem",
  },
  style5: {
    listStyleType: "decimal",
    paddingLeft: "1.5rem",
  },
  orderedListStart: (start: number) => ({
    counterReset: `ol-counter ${start - 1}`,
  }),
  style8: {
    minWidth: 0,
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
  },
  style9: {
    marginBlock: "1.5rem",
    overflowX: "auto",
  },
  style10: {
    width: "100%",
    borderCollapse: "collapse",
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    textAlign: "left",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
  },
  style11: {
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: ".75rem",
    paddingBlock: ".5rem",
    verticalAlign: "top",
  },
  style12: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: ".75rem",
    paddingBlock: ".5rem",
    verticalAlign: "top",
    fontWeight: 500,
  },
  style13: {
    marginBlock: "1rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style14: {
    marginBlock: "1.5rem",
  },
  style15: {
    maxHeight: "70vh",
    maxWidth: "100%",
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    objectFit: "contain",
  },
  style16: {
    marginBlock: "1.25rem",
  },
  style17: {
    marginBottom: ".5rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: colors.mutedForeground,
  },
  style18: {
    width: "100%",
  },
  style19: {
    marginBlock: "1rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1rem",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: ".75rem",
    borderStyle: "solid",
    borderWidth: "1px",
    paddingInline: "1rem",
    paddingBlock: ".75rem",
    color: colors.foreground,
    textDecorationLine: "none",
  },
  style20: {
    minWidth: 0,
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    overflow: "hidden",
    fontWeight: 500,
  },
  style21: {
    flexShrink: 0,
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
});
export type SharedAttachmentResolver = (
  attachment: SharedNoteAttachment,
  signal: AbortSignal,
) => Promise<SharedNoteAttachmentDownload | null>;
const AttachmentContext = createContext<{
  attachments: ReadonlyMap<string, SharedNoteAttachment>;
  excluded: ReadonlySet<string>;
  resolve: SharedAttachmentResolver | null;
}>({
  attachments: new Map(),
  excluded: new Set(),
  resolve: null,
});
export function SharedNoteDocument({
  attachments,
  document,
  excludedAttachmentIds = [],
  resolveAttachment,
}: {
  attachments: SharedNoteAttachment[];
  document: SharedNoteNode;
  excludedAttachmentIds?: readonly string[];
  resolveAttachment?: SharedAttachmentResolver;
}) {
  const context = useMemo(
    () => ({
      attachments: new Map(
        attachments.map((attachment) => [attachment.id, attachment]),
      ),
      excluded: new Set(excludedAttachmentIds),
      resolve: resolveAttachment ?? null,
    }),
    [attachments, excludedAttachmentIds, resolveAttachment],
  );
  const unreferencedAttachments = useMemo(() => {
    const referenced = collectSharedAttachmentIds(document);
    const excluded = new Set(excludedAttachmentIds);
    return attachments.filter(
      (attachment) =>
        !referenced.has(attachment.id) && !excluded.has(attachment.id),
    );
  }, [attachments, document, excludedAttachmentIds]);
  return (
    <AttachmentContext.Provider value={context}>
      <div
        {...mergeStyleXProps(
          styles.style1,
          "ProseMirror prosemirror-editor note-typography session-note-editor shared-note-document",
        )}
      >
        {renderChildren(document.content, "document")}
        {unreferencedAttachments.length > 0 ? (
          <section {...stylex.props(styles.style2)}>
            <h2>Attachments</h2>
            {unreferencedAttachments.map((attachment) => (
              <SharedAttachmentNode
                key={attachment.id}
                node={{
                  type: attachment.contentType.startsWith("audio/")
                    ? "clip"
                    : "fileAttachment",
                  attrs: {
                    sharedAttachmentId: attachment.id,
                  },
                }}
              />
            ))}
          </section>
        ) : null}
      </div>
    </AttachmentContext.Provider>
  );
}
function collectSharedAttachmentIds(root: SharedNoteNode) {
  const ids = new Set<string>();
  const visit = (node: SharedNoteNode) => {
    const id = node.attrs?.sharedAttachmentId;
    if (typeof id === "string") ids.add(id);
    node.content?.forEach(visit);
  };
  visit(root);
  return ids;
}
function renderChildren(nodes: SharedNoteNode[] | undefined, path: string) {
  return nodes?.map((node, index) => renderNode(node, `${path}-${index}`));
}
function renderNode(node: SharedNoteNode, key: string): ReactNode {
  const children = renderChildren(node.content, key);
  switch (node.type) {
    case "text":
      return <Fragment key={key}>{renderMarkedText(node, key)}</Fragment>;
    case "hardBreak":
      return <br key={key} />;
    case "paragraph":
      return <p key={key}>{children}</p>;
    case "heading": {
      const level = getIntegerAttr(node, "level", 1, 6, 2);
      return createElement(
        `h${level}`,
        {
          key,
        },
        children,
      );
    }
    case "blockquote":
      return <blockquote key={key}>{children}</blockquote>;
    case "codeBlock":
      return (
        <pre key={key}>
          <code>{children}</code>
        </pre>
      );
    case "horizontalRule":
      return <hr key={key} {...stylex.props(styles.style3)} />;
    case "image":
    case "fileAttachment":
    case "clip":
      return <SharedAttachmentNode key={key} node={node} />;
    case "bulletList":
      return (
        <ul key={key} {...stylex.props(styles.style4)}>
          {children}
        </ul>
      );
    case "orderedList": {
      const start = getIntegerAttr(node, "start", 1, 1_000_000, 1);
      return (
        <ol
          key={key}
          {...stylex.props(
            styles.style5,
            start !== 1 && styles.orderedListStart(start),
          )}
          start={start}
        >
          {children}
        </ol>
      );
    }
    case "listItem":
      return <li key={key}>{children}</li>;
    case "taskList":
      return (
        <ul key={key} data-type="taskList">
          {children}
        </ul>
      );
    case "taskItem": {
      const checked =
        node.attrs?.checked === true || node.attrs?.status === "done";
      return (
        <li key={key} data-checked={checked ? "true" : "false"}>
          <label className="task-checkbox-label">
            <input
              type="checkbox"
              checked={checked}
              disabled
              aria-label={checked ? "Completed task" : "Open task"}
              className="task-checkbox"
            />
          </label>
          <div {...stylex.props(styles.style8)}>{children}</div>
        </li>
      );
    }
    case "table":
      return (
        <div key={key} {...stylex.props(styles.style9)}>
          <table {...stylex.props(styles.style10)}>
            <tbody>{children}</tbody>
          </table>
        </div>
      );
    case "tableRow":
      return <tr key={key}>{children}</tr>;
    case "tableCell":
      return (
        <td
          key={key}
          colSpan={getIntegerAttr(node, "colspan", 1, 1000, 1)}
          rowSpan={getIntegerAttr(node, "rowspan", 1, 1000, 1)}
          {...stylex.props(styles.style11)}
        >
          {children}
        </td>
      );
    case "tableHeader":
      return (
        <th
          key={key}
          colSpan={getIntegerAttr(node, "colspan", 1, 1000, 1)}
          rowSpan={getIntegerAttr(node, "rowspan", 1, 1000, 1)}
          {...stylex.props(styles.style12)}
        >
          {children}
        </th>
      );
    default:
      return null;
  }
}
function SharedAttachmentNode({ node }: { node: SharedNoteNode }) {
  const { attachments, excluded, resolve } = useContext(AttachmentContext);
  const [pinnedAudioDownload, setPinnedAudioDownload] =
    useState<SharedNoteAttachmentDownload | null>(null);
  const [audioPlaying, setAudioPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement>(null);
  const sharedAttachmentId = node.attrs?.sharedAttachmentId;
  const attachment =
    typeof sharedAttachmentId === "string"
      ? attachments.get(sharedAttachmentId)
      : undefined;
  const isAudio = Boolean(
    attachment &&
    node.type === "clip" &&
    attachment.contentType.startsWith("audio/"),
  );
  const downloadQuery = useQuery({
    queryKey: ["shared-note-attachment-download", attachment?.id ?? ""],
    queryFn: ({ signal }) => resolve!(attachment!, signal),
    enabled: Boolean(attachment && resolve && !excluded.has(attachment.id)),
    retry: false,
    staleTime: 45_000,
    refetchInterval: audioPlaying ? false : 45_000,
    gcTime: 0,
  });
  const download =
    !downloadQuery.error &&
    attachment &&
    isMatchingSharedNoteAttachmentDownload(attachment, downloadQuery.data)
      ? downloadQuery.data
      : null;
  const activeDownload = isAudio ? (pinnedAudioDownload ?? download) : download;
  if (attachment && excluded.has(attachment.id)) {
    return null;
  }
  if (!attachment || !resolve || !activeDownload) {
    return (
      <div {...stylex.props(styles.style13)}>
        {downloadQuery.isPending && attachment && resolve
          ? `Loading ${attachment.filename}…`
          : "Attachment unavailable"}
      </div>
    );
  }
  if (node.type === "image" && isInlineImage(attachment.contentType)) {
    return (
      <figure {...stylex.props(styles.style14)}>
        <img
          src={activeDownload.signedUrl}
          alt={getStringAttr(node, "alt") ?? attachment.filename}
          loading="lazy"
          referrerPolicy="no-referrer"
          {...stylex.props(styles.style15)}
        />
      </figure>
    );
  }
  if (isAudio) {
    const refreshAudioGrant = async (
      audio: HTMLAudioElement,
      resume: boolean,
    ) => {
      const currentTime = audio.currentTime;
      audio.pause();
      const refreshed = await downloadQuery.refetch();
      if (
        refreshed.isError ||
        !isMatchingSharedNoteAttachmentDownload(attachment, refreshed.data)
      ) {
        return;
      }
      setPinnedAudioDownload(refreshed.data);
      requestAnimationFrame(() => {
        const current = audioRef.current;
        if (!current) return;
        current.currentTime = currentTime;
        if (resume) void current.play();
      });
    };
    return (
      <div {...stylex.props(styles.style16)}>
        <p {...stylex.props(styles.style17)}>{attachment.filename}</p>
        <audio
          ref={audioRef}
          controls
          preload="metadata"
          src={activeDownload.signedUrl}
          onPlay={(event) => {
            const current = pinnedAudioDownload ?? download;
            if (!current) {
              event.currentTarget.pause();
              return;
            }
            if (Date.parse(current.expiresAt) - Date.now() <= 10_000) {
              void refreshAudioGrant(event.currentTarget, true);
              return;
            }
            setPinnedAudioDownload(current);
            setAudioPlaying(true);
          }}
          onPause={() => setAudioPlaying(false)}
          onEnded={() => {
            setAudioPlaying(false);
            setPinnedAudioDownload(null);
          }}
          onError={(event) => {
            if (!downloadQuery.isFetching) {
              void refreshAudioGrant(event.currentTarget, audioPlaying);
            }
          }}
          {...stylex.props(styles.style18)}
        />
      </div>
    );
  }
  return (
    <a
      href={activeDownload.signedUrl}
      download={attachment.filename}
      target="_blank"
      rel="ugc noopener noreferrer"
      referrerPolicy="no-referrer"
      {...stylex.props(styles.style19)}
    >
      <span {...stylex.props(styles.style20)}>{attachment.filename}</span>
      <span {...stylex.props(styles.style21)}>
        {formatFileSize(attachment.sizeBytes)}
      </span>
    </a>
  );
}
function isInlineImage(contentType: string) {
  return [
    "image/avif",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
  ].includes(contentType);
}
function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
function getStringAttr(node: SharedNoteNode, name: string) {
  const value = node.attrs?.[name];
  return typeof value === "string" && value ? value : null;
}
function renderMarkedText(node: SharedNoteNode, key: string) {
  let content: ReactNode = node.text ?? "";
  for (const [index, mark] of (node.marks ?? []).entries()) {
    const markKey = `${key}-mark-${index}`;
    switch (mark.type) {
      case "bold":
        content = <strong key={markKey}>{content}</strong>;
        break;
      case "italic":
        content = <em key={markKey}>{content}</em>;
        break;
      case "strike":
        content = <s key={markKey}>{content}</s>;
        break;
      case "underline":
        content = <u key={markKey}>{content}</u>;
        break;
      case "highlight":
        content = <mark key={markKey}>{content}</mark>;
        break;
      case "code":
        content = <code key={markKey}>{content}</code>;
        break;
      case "link": {
        const href = getSafeSharedNoteHref(mark.attrs?.href);
        if (href) {
          content = (
            <a
              key={markKey}
              href={href}
              target="_blank"
              rel="ugc noopener noreferrer"
              referrerPolicy="no-referrer"
            >
              {content}
            </a>
          );
        }
        break;
      }
    }
  }
  return content;
}
function getIntegerAttr(
  node: SharedNoteNode,
  name: string,
  min: number,
  max: number,
  fallback: number,
) {
  const value = node.attrs?.[name];
  return typeof value === "number" &&
    Number.isInteger(value) &&
    value >= min &&
    value <= max
    ? value
    : fallback;
}
