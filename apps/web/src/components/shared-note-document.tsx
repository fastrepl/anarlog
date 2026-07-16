import { Fragment, createElement, type ReactNode } from "react";

import { getSafeSharedNoteHref, type SharedNoteNode } from "@/lib/shared-notes";

export function SharedNoteDocument({ document }: { document: SharedNoteNode }) {
  return (
    <div className="shared-note-document text-color text-base leading-7">
      {renderChildren(document.content, "document")}
    </div>
  );
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
