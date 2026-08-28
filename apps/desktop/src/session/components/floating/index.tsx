import { AnimatePresence, motion } from "motion/react";

import { cn } from "@anlg/utils";

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
    <div
      className={cn([
        "pointer-events-none absolute bottom-3 left-1/2 z-30 flex max-w-[calc(100%-2rem)] -translate-x-1/2 flex-col-reverse items-center",
      ])}
    >
      <div className="peer/session-fab pointer-events-auto relative h-10 w-[180px] max-w-full">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key="chat"
            aria-hidden={false}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeOut" }}
            className="visible relative max-w-full transition-transform duration-200 ease-out"
          >
            <ChatCTA />
          </motion.div>
        </AnimatePresence>
      </div>
      <div
        ref={setSessionFabSelectionHost}
        data-session-fab-selection
        className={cn([
          "pointer-events-auto z-10 mb-2",
          "origin-bottom transition-transform duration-150 ease-[cubic-bezier(0.22,1,0.36,1)]",
          "translate-y-8 dark:translate-y-7",
          "peer-focus-within/session-fab:translate-y-0 peer-hover/session-fab:translate-y-0",
          "dark:peer-focus-within/session-fab:translate-y-0 dark:peer-hover/session-fab:translate-y-0",
        ])}
      />
    </div>
  );
}
