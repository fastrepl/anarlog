import * as main from "~/store/tinybase/store/main";

export function useDevtoolsStore() {
  return main.UI.useStore(main.STORE_ID) as main.Store | undefined;
}

export function useDevtoolsUserId() {
  const { user_id } = main.UI.useValues(main.STORE_ID);
  return user_id;
}
