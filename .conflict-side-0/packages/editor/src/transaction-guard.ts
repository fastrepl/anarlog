import type { Transaction } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";

export type EditorTransactionErrorHandler = (
  error: unknown,
  view: EditorView,
  transaction: Transaction,
) => void;

export function dispatchEditorTransaction({
  view,
  transaction,
  onDocChanged,
  onError = logEditorTransactionError,
}: {
  view: EditorView;
  transaction: Transaction;
  onDocChanged?: (
    view: EditorView,
    transactions: readonly Transaction[],
  ) => void;
  onError?: EditorTransactionErrorHandler;
}) {
  try {
    const { state, transactions } = view.state.applyTransaction(transaction);
    view.updateState(state);

    if (transactions.some((tr) => tr.docChanged)) {
      onDocChanged?.(view, transactions);
    }
  } catch (error) {
    onError(error, view, transaction);
  }
}

export function logEditorTransactionError(error: unknown) {
  console.error("Editor transaction failed", error);
}
