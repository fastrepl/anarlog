export function isSelectAllShortcut(event: KeyboardEvent) {
  return (
    event.key.toLowerCase() === "a" &&
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey
  );
}

export function isDeleteSelectionShortcut(event: KeyboardEvent) {
  return (
    event.key === "Backspace" &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.shiftKey
  );
}

export function isSessionItemKey(key: string) {
  return key.startsWith("session-");
}

export function hasSidebarNoteSelectionContext({
  anchorId,
  selectedIds,
  selectedSessionId,
}: {
  anchorId: string | null;
  selectedIds: string[];
  selectedSessionId: string;
}) {
  const currentSessionKey = `session-${selectedSessionId}`;

  return anchorId === currentSessionKey || selectedIds.some(isSessionItemKey);
}

export function isTextEditingShortcutTarget(target: EventTarget | null) {
  const element =
    target instanceof Element
      ? target
      : target instanceof Node
        ? target.parentElement
        : null;

  return (
    element !== null &&
    Boolean(
      element.closest(
        [
          "input",
          "textarea",
          "select",
          "[contenteditable='true']",
          "[role='textbox']",
          ".ProseMirror",
        ].join(","),
      ),
    )
  );
}

export function scrollTimelineItemIntoView(
  container: HTMLDivElement | null,
  item: HTMLDivElement,
) {
  if (!container) {
    return;
  }

  const containerRect = container.getBoundingClientRect();
  const itemRect = item.getBoundingClientRect();
  const margin = 12;
  const aboveViewport = itemRect.top < containerRect.top + margin;
  const belowViewport = itemRect.bottom > containerRect.bottom - margin;

  if (!aboveViewport && !belowViewport) {
    return;
  }

  const itemCenter =
    itemRect.top -
    containerRect.top +
    container.scrollTop +
    itemRect.height / 2;
  const targetScrollTop = Math.max(
    itemCenter - container.clientHeight * 0.45,
    0,
  );

  container.scrollTo({
    top: targetScrollTop,
    behavior: "smooth",
  });
}

export function isTimelineItemVisible(
  container: HTMLDivElement | null,
  item: HTMLDivElement | null,
) {
  if (!container || !item) {
    return false;
  }

  const containerRect = container.getBoundingClientRect();
  const itemRect = item.getBoundingClientRect();
  const margin = 8;

  return (
    itemRect.bottom > containerRect.top + margin &&
    itemRect.top < containerRect.bottom - margin
  );
}
