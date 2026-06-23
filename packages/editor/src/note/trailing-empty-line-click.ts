import { Plugin, PluginKey, Selection, TextSelection } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

const trailingEmptyLineClickPluginKey = new PluginKey("trailingEmptyLineClick");

export function trailingEmptyLineClickPlugin() {
  return new Plugin({
    key: trailingEmptyLineClickPluginKey,
    props: {
      handleDOMEvents: {
        mousedown(view, event) {
          return handleTrailingEmptyLineMouseDown(view, event);
        },
      },
    },
  });
}

export function handleTrailingEmptyLineMouseDown(
  view: EditorView,
  event: MouseEvent,
) {
  if (
    event.defaultPrevented ||
    event.button !== 0 ||
    event.target !== view.dom
  ) {
    return false;
  }

  const lastBlock = view.dom.lastElementChild;
  if (lastBlock && event.clientY <= lastBlock.getBoundingClientRect().bottom) {
    return false;
  }

  const paragraph = view.state.schema.nodes.paragraph;
  if (!paragraph) {
    return false;
  }

  event.preventDefault();

  const { doc } = view.state;
  const lastChild = doc.lastChild;
  if (lastChild?.type === paragraph && !lastChild.textContent.trim()) {
    view.dispatch(view.state.tr.setSelection(Selection.atEnd(doc)));
    view.focus();
    return true;
  }

  const insertPos = doc.content.size;
  const tr = view.state.tr.insert(insertPos, paragraph.create());
  tr.setSelection(TextSelection.create(tr.doc, insertPos + 1));
  view.dispatch(tr.scrollIntoView());
  view.focus();
  return true;
}
