import * as main from "~/store/tinybase/store/main";

export type MeetingFloatMainStore = main.Store;

export function useMeetingFloatMainStore() {
  return main.UI.useStore(main.STORE_ID) as MeetingFloatMainStore | undefined;
}
