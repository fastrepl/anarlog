import * as stylex from "@stylexjs/stylex";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { CalendarSidebarContent } from "~/calendar/components/sidebar";

export function CalendarNav() {
  return (
    <div {...stylex.props(styles.root)}>
      <CustomSidebarHeader />
      <div {...stylex.props(styles.content)}>
        <CalendarSidebarContent />
      </div>
    </div>
  );
}

const styles = stylex.create({
  content: {
    "::-webkit-scrollbar": {
      display: "none",
    },
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    paddingInline: "0.75rem",
    scrollbarWidth: "none",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
    paddingBottom: "0.5rem",
  },
});
