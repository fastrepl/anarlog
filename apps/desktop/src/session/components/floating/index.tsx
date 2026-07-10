import { AnimatePresence, motion } from "motion/react";

import { cn } from "@hypr/utils";

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
    <div
      className={cn([
        "absolute left-1/2 z-30 flex max-w-[calc(100%-2rem)] -translate-x-1/2 items-end justify-center",
        "pointer-events-none bottom-3 h-10 w-[180px] pb-0",
      ])}
    >
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key="chat"
          aria-hidden={false}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="pointer-events-auto visible relative max-w-full transition-transform duration-200 ease-out"
        >
          <ChatCTA />
        </motion.div>
      </AnimatePresence>
    </div>
  );
}
