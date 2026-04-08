import { CheckIcon, MinusIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import type { TaskStatus } from "../tasks";

type TaskCheckboxProps = {
  status: TaskStatus;
  isInteractive?: boolean;
  isSelected?: boolean;
  onToggle?: () => void;
};

export function TaskCheckbox({
  status,
  isInteractive = false,
  isSelected = false,
  onToggle,
}: TaskCheckboxProps) {
  const ariaChecked =
    status === "done" ? true : status === "in_progress" ? "mixed" : false;

  return (
    <label
      className="task-checkbox-label"
      contentEditable={false}
      suppressContentEditableWarning
    >
      <button
        type="button"
        role="checkbox"
        aria-checked={ariaChecked}
        className="task-checkbox"
        data-status={status}
        data-interactive={isInteractive ? "true" : "false"}
        data-selected={isSelected ? "true" : undefined}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();

          if (isInteractive) {
            onToggle?.();
          }
        }}
        onMouseDown={(event) => {
          event.preventDefault();
          event.stopPropagation();
        }}
      >
        <span
          className={cn([
            "pointer-events-none flex size-full items-center justify-center text-white",
            status === "todo" && "text-transparent",
          ])}
        >
          {status === "done" ? (
            <CheckIcon size={12} strokeWidth={3} />
          ) : status === "in_progress" ? (
            <MinusIcon size={12} strokeWidth={3} />
          ) : null}
        </span>
      </button>
    </label>
  );
}
