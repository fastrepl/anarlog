import { useTabs } from "~/store/zustand/tabs";
import { TemplatesSidebarContent } from "~/templates";

export function TemplatesNav() {
  const currentTab = useTabs((state) => state.currentTab);

  if (currentTab?.type !== "templates") {
    return null;
  }

  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <TemplatesSidebarContent tab={currentTab} />
    </div>
  );
}
