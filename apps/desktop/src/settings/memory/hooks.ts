import * as main from "~/store/tinybase/store/main";

export type VocabItem = {
  rowId: string;
  text: string;
};

export type VocabMutations = {
  create: (text: string) => void;
  update: (rowId: string, text: string) => void;
  delete: (rowId: string) => void;
};

export function useVocabs(): VocabItem[] {
  const table = main.UI.useResultTable(
    main.QUERIES.visibleVocabs,
    main.STORE_ID,
  );

  return Object.entries(table ?? {}).map(([rowId, { text }]) => ({
    rowId,
    text: (text as string | undefined) ?? "",
  }));
}

export function useVocabMutations(): VocabMutations {
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const createRow = main.UI.useSetRowCallback(
    "memories",
    () => crypto.randomUUID(),
    (text: string) => ({
      user_id: user_id!,
      type: "vocab",
      text,
      created_at: new Date().toISOString(),
    }),
    [user_id],
    main.STORE_ID,
  );

  const updateRow = main.UI.useSetPartialRowCallback(
    "memories",
    ({ rowId }: { rowId: string; text: string }) => rowId,
    ({ text }: { rowId: string; text: string }) => ({ text }),
    [],
    main.STORE_ID,
  ) as (args: { rowId: string; text: string }) => void;

  const deleteRow = main.UI.useDelRowCallback(
    "memories",
    (rowId: string) => rowId,
    main.STORE_ID,
  );

  return {
    create: (text: string) => {
      if (!user_id) return;
      createRow(text);
    },
    update: (rowId: string, text: string) => {
      updateRow({ rowId, text });
    },
    delete: (rowId: string) => {
      deleteRow(rowId);
    },
  };
}
