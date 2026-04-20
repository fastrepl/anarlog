import { AppProvidersView } from "~/app/app-providers.view";
import { useAppLifecycle } from "~/app/use-app-lifecycle";
import { HomeContainer } from "~/home";
import { ShellContainer } from "~/shell";
import { selectCurrentTab, TabContentView, useTabsStore } from "~/tabs";

/**
 * Composition root mounted by the router's root route.
 *
 * Mirrors the Tauri `/app/main2/_layout` route (see
 * `apps/desktop/src/routes/app/main2/_layout.tsx` +
 * `apps/desktop/src/main2/layout.tsx`): run app-wide lifecycle, install the
 * provider stack, then render the shell with tab-aware body content.
 *
 * Keep this container thin. Provider wiring belongs in `AppProvidersView`,
 * effect wiring in `useAppLifecycle`. Tab → body resolution stays here because
 * it is the only place that knows the shell shape.
 */
export function AppRootContainer() {
  useAppLifecycle();

  const currentTab = useTabsStore(selectCurrentTab);

  return (
    <AppProvidersView>
      <ShellContainer
        body={
          <TabContentView tab={currentTab} homeContent={<HomeContainer />} />
        }
      />
    </AppProvidersView>
  );
}
