type TaskCheckboxProps = {
  checked: boolean;
  isInteractive?: boolean;
  isSelected?: boolean;
  onToggle?: () => void;
};

export function TaskCheckbox({
  checked,
  isInteractive = false,
  isSelected = false,
  onToggle,
}: TaskCheckboxProps) {
  return (
    <label
      className="task-checkbox-label"
      contentEditable={false}
      suppressContentEditableWarning
    >
      <input
        type="checkbox"
        className="task-checkbox"
        checked={checked}
        readOnly
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
      />
    </label>
  );
}
