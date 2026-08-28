import * as stylex from "@stylexjs/stylex";

import { useTabs } from "~/store/zustand/tabs";
import { TemplatesSidebarContent } from "~/templates";

export function TemplatesNav() {
  const currentTab = useTabs((state) => state.currentTab);

  if (currentTab?.type !== "templates") {
    return null;
  }

  return (
    <div {...stylex.props(styles.root)}>
      <TemplatesSidebarContent tab={currentTab} />
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    paddingBottom: "0.5rem",
  },
});
