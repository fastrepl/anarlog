import { useShell } from "~/contexts/shell";
import { useConfigValue } from "~/shared/config";
import { LeftSidebar } from "~/sidebar";
import {
  hasCustomSidebarTab,
  useCustomSidebarEffect,
} from "~/sidebar/use-custom-sidebar";
import { useTabs } from "~/store/zustand/tabs";

export function ClassicMainSidebar() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);
  const sidebarTimelineEnabled = useConfigValue("sidebar_timeline_enabled");
  const isOnboarding = currentTab?.type === "onboarding";

  const hasCustomSidebar = hasCustomSidebarTab(currentTab);

  useCustomSidebarEffect(hasCustomSidebar, leftsidebar);

  if (!leftsidebar.expanded || isOnboarding) {
    return null;
  }

  if (hasCustomSidebar || sidebarTimelineEnabled) {
    return <LeftSidebar />;
  }

  return null;
}
