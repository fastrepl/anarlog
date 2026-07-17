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

import {
  getSafeSharedNoteHref,
  type SharedNoteAttachment,
  type SharedNoteAttachmentDownload,
  type SharedNoteNode,
} from "@/lib/shared-notes";

export type SharedAttachmentResolver = (
  attachment: SharedNoteAttachment,
  signal: AbortSignal,
) => Promise<SharedNoteAttachmentDownload | null>;

const AttachmentContext = createContext<{
  attachments: ReadonlyMap<string, SharedNoteAttachment>;
  resolve: SharedAttachmentResolver | null;
}>({ attachments: new Map(), resolve: null });

export function SharedNoteDocument({
  attachments,
  document,
  resolveAttachment,
}: {
  attachments: SharedNoteAttachment[];
  document: SharedNoteNode;
  resolveAttachment?: SharedAttachmentResolver;
}) {
  const context = useMemo(
    () => ({
      attachments: new Map(
        attachments.map((attachment) => [attachment.id, attachment]),
      ),
      resolve: resolveAttachment ?? null,
    }),
    [attachments, resolveAttachment],
  );
  const unreferencedAttachments = useMemo(() => {
    const referenced = collectSharedAttachmentIds(document);
    return attachments.filter((attachment) => !referenced.has(attachment.id));
  }, [attachments, document]);
  return (
    <AttachmentContext.Provider value={context}>
      <div className="shared-note-document text-color text-base leading-7">
        {renderChildren(document.content, "document")}
        {unreferencedAttachments.length > 0 ? (
          <section className="border-color-subtle mt-10 border-t pt-6">
            <h2 className="mb-3 font-mono text-sm font-medium">Attachments</h2>
            {unreferencedAttachments.map((attachment) => (
              <SharedAttachmentNode
                key={attachment.id}
                node={{
                  type: attachment.contentType.startsWith("audio/")
                    ? "clip"
                    : "fileAttachment",
                  attrs: { sharedAttachmentId: attachment.id },
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
      return (
        <p key={key} className="my-4 text-base leading-7">
          {children}
        </p>
      );
    case "heading": {
      const level = getIntegerAttr(node, "level", 1, 6, 2);
      return createElement(
        `h${level}`,
        {
          key,
          className: "font-mono mt-10 mb-4 text-xl font-medium first:mt-0",
        },
        children,
      );
    }
    case "blockquote":
      return (
        <blockquote
          key={key}
          className="border-color-brand text-color-muted my-6 border-l-2 pl-5"
        >
          {children}
        </blockquote>
      );
    case "codeBlock":
      return (
        <pre
          key={key}
          className="surface-subtle my-6 overflow-x-auto rounded-xl p-4 font-mono text-sm leading-6"
        >
          <code>{children}</code>
        </pre>
      );
    case "horizontalRule":
      return <hr key={key} className="border-color-subtle my-8 border-t" />;
    case "image":
    case "fileAttachment":
    case "clip":
      return <SharedAttachmentNode key={key} node={node} />;
    case "bulletList":
      return (
        <ul key={key} className="my-4 list-disc space-y-1 pl-6">
          {children}
        </ul>
      );
    case "orderedList":
      return (
        <ol
          key={key}
          start={getIntegerAttr(node, "start", 1, 1_000_000, 1)}
          className="my-4 list-decimal space-y-1 pl-6"
        >
          {children}
        </ol>
      );
    case "listItem":
      return (
        <li key={key} className="pl-1 text-base leading-7">
          {children}
        </li>
      );
    case "taskList":
      return (
        <ul key={key} className="my-4 list-none space-y-2 pl-0">
          {children}
        </ul>
      );
    case "taskItem": {
      const checked =
        node.attrs?.checked === true || node.attrs?.status === "done";
      return (
        <li key={key} className="flex items-start gap-3 text-base leading-7">
          <input
            type="checkbox"
            checked={checked}
            disabled
            aria-label={checked ? "Completed task" : "Open task"}
            className="border-color-brand mt-1.5 size-4 shrink-0 rounded"
          />
          <div className="min-w-0 flex-1">{children}</div>
        </li>
      );
    }
    case "table":
      return (
        <div key={key} className="my-6 overflow-x-auto">
          <table className="border-color-subtle w-full border-collapse border text-left text-sm">
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
          className="border-color-subtle border px-3 py-2 align-top"
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
          className="surface-subtle border-color-subtle border px-3 py-2 align-top font-medium"
        >
          {children}
        </th>
      );
    default:
      return null;
  }
}

function SharedAttachmentNode({ node }: { node: SharedNoteNode }) {
  const { attachments, resolve } = useContext(AttachmentContext);
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
    enabled: Boolean(attachment && resolve),
    retry: false,
    staleTime: 45_000,
    refetchInterval: audioPlaying ? false : 45_000,
    gcTime: 0,
  });
  const download =
    !downloadQuery.error &&
    attachment &&
    isMatchingDownload(attachment, downloadQuery.data)
      ? downloadQuery.data
      : null;
  const activeDownload = isAudio ? (pinnedAudioDownload ?? download) : download;

  if (!attachment || !resolve || !activeDownload) {
    return (
      <div className="surface-subtle border-color-subtle text-color-muted my-4 rounded-xl border px-4 py-3 text-sm">
        {downloadQuery.isPending && attachment && resolve
          ? `Loading ${attachment.filename}…`
          : "Attachment unavailable"}
      </div>
    );
  }

  if (node.type === "image" && isInlineImage(attachment.contentType)) {
    return (
      <figure className="my-6">
        <img
          src={activeDownload.signedUrl}
          alt={getStringAttr(node, "alt") ?? attachment.filename}
          loading="lazy"
          referrerPolicy="no-referrer"
          className="border-color-subtle max-h-[70vh] max-w-full rounded-xl border object-contain"
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
        !isMatchingDownload(attachment, refreshed.data)
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
      <div className="my-5">
        <p className="text-color-muted mb-2 text-sm">{attachment.filename}</p>
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
          className="w-full"
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
      className="surface-subtle border-color-subtle text-color my-4 flex items-center justify-between gap-4 rounded-xl border px-4 py-3 no-underline"
    >
      <span className="min-w-0 truncate font-medium">
        {attachment.filename}
      </span>
      <span className="text-color-muted shrink-0 text-xs">
        {formatFileSize(attachment.sizeBytes)}
      </span>
    </a>
  );
}

function isMatchingDownload(
  attachment: SharedNoteAttachment,
  download: SharedNoteAttachmentDownload | null | undefined,
): download is SharedNoteAttachmentDownload {
  return Boolean(
    download &&
    download.id === attachment.id &&
    download.filename === attachment.filename &&
    download.contentType === attachment.contentType &&
    download.sizeBytes === attachment.sizeBytes &&
    download.sha256 === attachment.sha256,
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
      case "highlight":
        content = (
          <mark
            key={markKey}
            className="brand-yellow text-color rounded px-0.5"
          >
            {content}
          </mark>
        );
        break;
      case "code":
        content = (
          <code
            key={markKey}
            className="surface-subtle rounded px-1.5 py-0.5 font-mono text-sm"
          >
            {content}
          </code>
        );
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
              className="text-color underline decoration-current underline-offset-2"
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
