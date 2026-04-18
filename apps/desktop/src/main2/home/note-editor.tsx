import "./note-editor.css";

import { useCallback, useEffect, useMemo, useRef } from "react";

import { format, parseISO, subDays } from "@hypr/utils";

import { useDailyNoteEditorRuntime } from "./note-editor-runtime";

import { useCalendarData } from "~/calendar/hooks";
import {
  type JSONContent,
  NoteEditor,
  type NoteEditorRef,
} from "~/editor/session";
import { useTaskStorageOptional } from "~/tasks/hooks";

function isEditorTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement && target.closest(".ProseMirror") !== null
  );
}

export function DailyNoteEditor({
  date,
  isToday,
}: {
  date: string;
  isToday?: boolean;
}) {
  const dailyNoteRuntime = useDailyNoteEditorRuntime(date);
  const editorRef = useRef<NoteEditorRef>(null);
  const taskStorage = useTaskStorageOptional();
  const taskSource = useMemo(() => ({ type: "daily_note", id: date }), [date]);
  const previousDate = useMemo(
    () => format(subDays(parseISO(`${date}T00:00:00`), 1), "yyyy-MM-dd"),
    [date],
  );
  const previousTaskSource = useMemo(
    () => ({ type: "daily_note", id: previousDate }),
    [previousDate],
  );

  const { eventIdsByDate, sessionIdsByDate } = useCalendarData();
  const eventIds = eventIdsByDate[date] ?? [];
  const sessionIds = sessionIdsByDate[date] ?? [];

  // Compute initial content once on mount — imperative read, no subscription.
  // This breaks the read→derive→write→read loop that `useCell` would create.
  const initialContentRef = useRef<JSONContent | null>(null);
  if (!initialContentRef.current) {
    initialContentRef.current = dailyNoteRuntime.initializeContent({
      isToday,
      previousDate,
      eventIds,
      sessionIds,
      taskStorage,
      taskSource,
      previousTaskSource,
    });
  }

  useEffect(() => {
    const view = editorRef.current?.view;
    if (!view) {
      return;
    }

    try {
      dailyNoteRuntime.syncLinkedSessionsInView(view, eventIds, sessionIds);
    } catch {
      // invalid content
    }
  }, [dailyNoteRuntime, eventIds, sessionIds]);

  const handleChange = useCallback(
    (input: JSONContent) => {
      dailyNoteRuntime.syncSessionNodeTitles(input);
      dailyNoteRuntime.persistDailyNote(input);
    },
    [dailyNoteRuntime],
  );

  const focusEditor = useCallback(() => {
    editorRef.current?.commands.focus();
  }, []);

  const handleContainerMouseDownCapture = useCallback(
    (event: React.MouseEvent) => {
      if (isEditorTarget(event.target)) {
        return;
      }

      event.preventDefault();
      focusEditor();
    },
    [focusEditor],
  );

  const handleContainerClick = useCallback(
    (event: React.MouseEvent) => {
      if (isEditorTarget(event.target)) {
        return;
      }

      focusEditor();
    },
    [focusEditor],
  );

  if (!initialContentRef.current) {
    return null;
  }

  return (
    <div
      className="main2-daily-note-editor flex-1 px-6"
      onMouseDownCapture={handleContainerMouseDownCapture}
      onClick={handleContainerClick}
    >
      <NoteEditor
        ref={editorRef}
        key={`daily-${date}`}
        initialContent={initialContentRef.current}
        handleChange={handleChange}
        linkedItemOpenBehavior="new"
        taskSource={taskSource}
      />
    </div>
  );
}
