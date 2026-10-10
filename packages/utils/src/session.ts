export type SessionNoteView =
  | { type: "raw" }
  | { type: "enhanced"; id: string }
  | { type: "transcript" }
  | { type: "attachments" }
  | { type: "shared"; id: string };

export function computeCurrentNoteTab(
  tabView: SessionNoteView | null,
  isLiveSessionActive: boolean,
  enhancedNoteIds: readonly string[],
  canShowTranscript = false,
  sharedNoteIds: readonly string[] = [],
): SessionNoteView {
  if (tabView?.type === "shared" && sharedNoteIds.includes(tabView.id)) {
    return tabView;
  }
  const firstEnhancedNoteId = enhancedNoteIds[0];
  const hasEnhancedNote = (id: string) => enhancedNoteIds.includes(id);

  if (isLiveSessionActive) {
    if (tabView?.type === "raw") {
      return tabView;
    }
    if (tabView?.type === "enhanced" && hasEnhancedNote(tabView.id)) {
      return tabView;
    }
    if (tabView?.type === "transcript" && canShowTranscript) {
      return tabView;
    }
    return { type: "raw" };
  }

  if (tabView) {
    if (tabView.type === "raw") {
      return tabView;
    }
    if (tabView.type === "enhanced") {
      return hasEnhancedNote(tabView.id)
        ? tabView
        : firstEnhancedNoteId
          ? { type: "enhanced", id: firstEnhancedNoteId }
          : { type: "raw" };
    }
    if (tabView.type === "transcript" && canShowTranscript) {
      return tabView;
    }

    return { type: "raw" };
  }

  if (firstEnhancedNoteId) {
    return { type: "enhanced", id: firstEnhancedNoteId };
  }

  return { type: "raw" };
}

export function createEditorTabs({
  enhancedNoteIds,
  canShowTranscript,
  sharedNoteIds = [],
}: {
  enhancedNoteIds: string[];
  canShowTranscript: boolean;
  sharedNoteIds?: readonly string[];
}): SessionNoteView[] {
  const enhancedTabs: SessionNoteView[] = enhancedNoteIds.map((id) => ({
    type: "enhanced",
    id,
  }));

  return [
    ...enhancedTabs,
    { type: "raw" },
    ...(canShowTranscript ? [{ type: "transcript" } as const] : []),
    ...sharedNoteIds.map((id) => ({ type: "shared", id }) as const),
  ];
}

const TEXT_CONTAINER_TYPES = new Set([
  "doc",
  "heading",
  "paragraph",
  "text",
  "codeBlock",
  "blockquote",
  "bulletList",
  "orderedList",
  "listItem",
]);

type TiptapNode = {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: TiptapNode[];
  marks?: Array<{ type?: string; attrs?: Record<string, unknown> }>;
  text?: string;
};

function hasMeaningfulTiptapContent(node: TiptapNode): boolean {
  if (typeof node.text === "string" && node.text.trim()) {
    return true;
  }

  if (!node.type || !TEXT_CONTAINER_TYPES.has(node.type)) {
    return true;
  }

  return node.content?.some(hasMeaningfulTiptapContent) ?? false;
}

function collectTiptapText(node: TiptapNode): string {
  const text = typeof node.text === "string" ? node.text : "";
  return text + (node.content?.map(collectTiptapText).join("") ?? "");
}

export function hasSummaryContent(
  value: unknown,
  sessionTitle?: string,
): boolean {
  if (typeof value !== "string") {
    return false;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return false;
  }

  if (!trimmed.startsWith("{")) {
    return true;
  }

  try {
    const parsed = JSON.parse(trimmed);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      (parsed as { type?: unknown }).type === "doc"
    ) {
      const document = parsed as TiptapNode;
      const blocks = document.content ?? [];
      const firstBlock = blocks[0];
      const firstBlockAttrs = firstBlock?.attrs ?? {};
      const synthesizedTitle =
        sessionTitle?.trim() &&
        firstBlock?.type === "heading" &&
        firstBlockAttrs.level === 1 &&
        Object.keys(firstBlockAttrs).length === 1 &&
        collectTiptapText(firstBlock).trim() === sessionTitle.trim() &&
        !firstBlock.content?.some(
          (child) =>
            child.type !== "text" ||
            !child.text?.trim() ||
            Boolean(child.marks?.length),
        );
      return (synthesizedTitle ? blocks.slice(1) : blocks).some(
        hasMeaningfulTiptapContent,
      );
    }
    return true;
  } catch {
    return true;
  }
}

export function defaultSummaryDocumentId(sessionId: string): string {
  return `summary:${sessionId}`;
}

export function resolveSummaryDocument<
  T extends {
    id: string;
    body: string;
    kind: string;
    template_id: string;
    sort_order: number;
  },
>(
  documents: readonly T[],
  sessionTitle?: string,
  templateId?: string,
): T | undefined {
  return [...documents].sort(
    (left, right) =>
      Number(hasSummaryContent(right.body, sessionTitle)) -
        Number(hasSummaryContent(left.body, sessionTitle)) ||
      Number(right.template_id === (templateId ?? "")) -
        Number(left.template_id === (templateId ?? "")) ||
      Number(left.kind !== "summary") - Number(right.kind !== "summary") ||
      left.sort_order - right.sort_order ||
      left.id.localeCompare(right.id),
  )[0];
}

export function visibleSummaryDocuments<
  T extends {
    id: string;
    body: string;
    kind: string;
    template_id: string;
    title: string;
    created_at: string;
    updated_at: string;
    generation_metadata_json: string;
  },
>(documents: readonly T[]): T[] {
  return documents.filter((document) => {
    const untouchedPlaceholder =
      document.kind === "summary" &&
      document.template_id === "" &&
      document.body === "" &&
      document.title === "Summary" &&
      document.created_at === document.updated_at &&
      document.generation_metadata_json === "{}";
    // Keep the row: a later body field from sync must bring its tab back.
    return (
      !untouchedPlaceholder ||
      !documents.some(
        (other) =>
          other.id !== document.id &&
          other.kind === "summary" &&
          other.template_id === "" &&
          hasSummaryContent(other.body),
      )
    );
  });
}
