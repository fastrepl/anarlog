import * as main from "~/store/tinybase/store/main";

export type MainStore = NonNullable<ReturnType<typeof main.UI.useStore>>;
export type MainIndexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;
export type MainQueries = ReturnType<typeof main.UI.useQueries>;
export type SliceIndexReader = Pick<MainIndexes, "getSliceRowIds">;

export function useMainStoreInternal(): MainStore | undefined {
  return main.UI.useStore(main.STORE_ID);
}

export function useMainIndexesInternal(): MainIndexes | undefined {
  return main.UI.useIndexes(main.STORE_ID);
}

export function useMainQueriesInternal(): MainQueries {
  return main.UI.useQueries(main.STORE_ID);
}

export function useCurrentUserId(): string | undefined {
  return main.UI.useValue("user_id", main.STORE_ID) as string | undefined;
}

export function useAiLanguage(): string {
  return (
    (main.UI.useValue("ai_language", main.STORE_ID) as string | undefined) ??
    "en"
  );
}
