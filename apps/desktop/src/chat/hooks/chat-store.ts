import * as main from "~/store/tinybase/store/main";

export function useChatStore() {
  return main.UI.useStore(main.STORE_ID);
}

export function useCurrentUserId(): string | undefined {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  return user_id;
}

export function useAiLanguage(): string {
  return (
    (main.UI.useValue("ai_language", main.STORE_ID) as string | undefined) ??
    "en"
  );
}

export const ENHANCED_NOTES_BY_SESSION_INDEX =
  main.INDEXES.enhancedNotesBySession;
