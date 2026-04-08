import { getRenderedAttributes } from "@tiptap/core";
import BaseTaskItem from "@tiptap/extension-task-item";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

const TaskItem = BaseTaskItem.extend({
  addNodeView() {
    return ({ node, HTMLAttributes, getPos, editor }) => {
      const listItem = document.createElement("li");
      const checkboxWrapper = document.createElement("label");
      const checkboxStyler = document.createElement("span");
      const checkbox = document.createElement("input");
      const content = document.createElement("div");

      const updateA11Y = (currentNode: ProseMirrorNode) => {
        checkbox.ariaLabel =
          this.options.a11y?.checkboxLabel?.(currentNode, checkbox.checked) ||
          `Task item checkbox for ${currentNode.textContent || "empty task item"}`;
      };

      updateA11Y(node);

      // Chrome can fail to paint full-document selections across task items when
      // the checkbox wrapper is marked contenteditable="false".
      checkbox.type = "checkbox";
      checkbox.addEventListener("mousedown", (event) => event.preventDefault());
      checkbox.addEventListener("change", (event) => {
        if (!editor.isEditable && !this.options.onReadOnlyChecked) {
          checkbox.checked = !checkbox.checked;
          return;
        }

        const { checked } = event.target as HTMLInputElement;

        if (editor.isEditable && typeof getPos === "function") {
          editor
            .chain()
            .focus(undefined, { scrollIntoView: false })
            .command(({ tr }) => {
              const position = getPos();

              if (typeof position !== "number") {
                return false;
              }

              const currentNode = tr.doc.nodeAt(position);

              tr.setNodeMarkup(position, undefined, {
                ...currentNode?.attrs,
                checked,
              });

              return true;
            })
            .run();
        }

        if (!editor.isEditable && this.options.onReadOnlyChecked) {
          if (!this.options.onReadOnlyChecked(node, checked)) {
            checkbox.checked = !checkbox.checked;
          }
        }
      });

      Object.entries(this.options.HTMLAttributes).forEach(([key, value]) => {
        listItem.setAttribute(key, value);
      });

      listItem.dataset.checked = node.attrs.checked;
      checkbox.checked = node.attrs.checked;
      checkboxWrapper.append(checkbox, checkboxStyler);
      listItem.append(checkboxWrapper, content);

      Object.entries(HTMLAttributes).forEach(([key, value]) => {
        listItem.setAttribute(key, value);
      });

      let prevRenderedAttributeKeys = new Set(Object.keys(HTMLAttributes));

      return {
        dom: listItem,
        contentDOM: content,
        update: (updatedNode) => {
          if (updatedNode.type !== this.type) {
            return false;
          }

          listItem.dataset.checked = updatedNode.attrs.checked;
          checkbox.checked = updatedNode.attrs.checked;
          updateA11Y(updatedNode);

          const extensionAttributes = editor.extensionManager.attributes;
          const newHTMLAttributes = getRenderedAttributes(
            updatedNode,
            extensionAttributes,
          );
          const newKeys = new Set(Object.keys(newHTMLAttributes));
          const staticAttrs = this.options.HTMLAttributes;

          prevRenderedAttributeKeys.forEach((key) => {
            if (!newKeys.has(key)) {
              if (key in staticAttrs) {
                listItem.setAttribute(key, staticAttrs[key]);
              } else {
                listItem.removeAttribute(key);
              }
            }
          });

          Object.entries(newHTMLAttributes).forEach(([key, value]) => {
            if (value === null || value === undefined) {
              if (key in staticAttrs) {
                listItem.setAttribute(key, staticAttrs[key]);
              } else {
                listItem.removeAttribute(key);
              }
            } else {
              listItem.setAttribute(key, value);
            }
          });

          prevRenderedAttributeKeys = newKeys;

          return true;
        },
      };
    };
  },
});

export default TaskItem;
