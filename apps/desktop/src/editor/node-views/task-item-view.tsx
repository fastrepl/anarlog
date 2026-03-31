import {
  type NodeViewComponentProps,
  useEditorEventCallback,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import { forwardRef, type ReactNode } from "react";

export const TaskItemView = forwardRef<
  HTMLLIElement,
  NodeViewComponentProps & { children?: ReactNode }
>(function TaskItemView({ nodeProps, children, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const checked = node.attrs.checked;

  const pos = getPos();
  const { selection } = useEditorState();
  const isSelected =
    pos >= selection.from && pos + node.nodeSize <= selection.to - 1;

  const handleChange = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getPos();
    const tr = view.state.tr.setNodeMarkup(pos, undefined, {
      ...node.attrs,
      checked: !checked,
    });
    view.dispatch(tr);
  });

  return (
    <li
      ref={ref}
      {...htmlAttrs}
      data-type="taskItem"
      data-checked={checked ? "true" : "false"}
    >
      <label contentEditable={false} suppressContentEditableWarning>
        <input
          type="checkbox"
          checked={checked}
          onChange={handleChange}
          onMouseDown={(e) => e.preventDefault()}
          data-selected={isSelected ? "true" : undefined}
        />
      </label>
      <div>{children}</div>
    </li>
  );
});
