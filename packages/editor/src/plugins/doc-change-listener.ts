import { Plugin, PluginKey } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

const docChangedByTransactionKey = new PluginKey<boolean>(
  "docChangedByTransaction",
);

export function docChangeListenerPlugin(
  onDocChanged: (view: EditorView) => void,
) {
  return new Plugin({
    key: docChangedByTransactionKey,
    state: {
      init: () => false,
      apply: (transaction, previous) => transaction.docChanged || previous,
    },
    view() {
      return {
        update(view, prevState) {
          if (prevState.doc === view.state.doc) {
            return;
          }

          if (!docChangedByTransactionKey.getState(view.state)) {
            return;
          }

          onDocChanged(view);
        },
      };
    },
  });
}
