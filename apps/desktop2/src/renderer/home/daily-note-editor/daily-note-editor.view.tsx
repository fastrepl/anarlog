import "@hypr/editor/daily/styles.css";
import { DailyNoteEditor } from "@hypr/editor/daily";

export function DailyNoteEditorView({
  value,
  onChange,
  status,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  status: "idle" | "saving";
  placeholder?: string;
}) {
  return (
    <div className="main2-daily-note-editor flex-1 px-6">
      <div className="mb-2 flex justify-end">
        <span className="text-xs text-neutral-400">
          {status === "saving" ? "Saving..." : "Saved"}
        </span>
      </div>
      <DailyNoteEditor
        value={value}
        onChange={onChange}
        placeholder={placeholder ?? "Write today's notes..."}
      />
    </div>
  );
}
