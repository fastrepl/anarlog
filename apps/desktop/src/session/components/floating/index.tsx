import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, motion } from "motion/react";

import { setSessionFabSelectionHost } from "./selection-slot";

import { ChatCTA } from "~/shared/chat-cta";
import type { EditorView, Tab } from "~/store/zustand/tabs/schema";

export function FloatingActionButton(_props: {
  allowListening?: boolean;
  audioExists?: boolean;
  currentView: EditorView;
  skipReason?: string | null;
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.fab, stylex.defaultMarker())}>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key="chat"
            aria-hidden={false}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            {...stylex.props(styles.chat)}
          >
            <ChatCTA />
          </motion.div>
        </AnimatePresence>
      </div>
      <div
        ref={setSessionFabSelectionHost}
        data-session-fab-selection
        {...stylex.props(styles.selection)}
      />
    </div>
  );
}

const styles = stylex.create({
  chat: {
    maxWidth: "100%",
    position: "relative",
    transitionDuration: "200ms",
    transitionProperty: "transform",
    transitionTimingFunction: "ease-out",
    visibility: "visible",
  },
  fab: {
    height: "2.5rem",
    maxWidth: "100%",
    pointerEvents: "auto",
    position: "relative",
    width: "180px",
  },
  root: {
    alignItems: "center",
    bottom: "0.75rem",
    display: "flex",
    flexDirection: "column-reverse",
    left: "50%",
    maxWidth: "calc(100% - 2rem)",
    pointerEvents: "none",
    position: "absolute",
    transform: "translateX(-50%)",
    zIndex: 30,
  },
  selection: {
    marginBottom: "0.5rem",
    pointerEvents: "auto",
    transform: {
      default: "translateY(2rem)",
      [stylex.when.siblingBefore(":focus-within")]: "translateY(0)",
      [stylex.when.siblingBefore(":hover")]: "translateY(0)",
    },
    transformOrigin: "bottom",
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
    zIndex: 10,
  },
});

export { styles as floatingActionButtonStyles };
