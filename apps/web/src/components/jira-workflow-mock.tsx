import { Icon } from "@iconify-icon/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import { JiraToolCall } from "./ai-feature-panel";

const CYCLE_MS = 6500;

export function JiraWorkflowMock() {
  const [step, setStep] = useState(0);
  const [loopKey, setLoopKey] = useState(0);

  useEffect(() => {
    setStep(0);
    const t1 = setTimeout(() => setStep(1), 400);
    const t2 = setTimeout(() => setStep(2), 1200);
    const t3 = setTimeout(() => setStep(3), 3800);
    const restart = setTimeout(() => setLoopKey((k) => k + 1), CYCLE_MS);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
      clearTimeout(restart);
    };
  }, [loopKey]);

  return (
    <div className="border-color-brand surface flex w-full flex-col overflow-hidden rounded-xl border shadow-xl">
      <div className="border-color-brand flex h-9 shrink-0 items-center border-b px-3">
        <div className="flex items-center gap-2">
          <Icon
            icon="mdi:message-text-outline"
            className="text-sm text-neutral-400"
          />
          <span className="text-xs font-medium text-neutral-700">Chat</span>
        </div>
      </div>

      <div className="flex min-h-[340px] flex-col justify-end gap-3 p-3">
        <AnimatePresence mode="popLayout">
          {step >= 1 && (
            <motion.div
              key={`q-${loopKey}`}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2 }}
              className="flex w-full justify-end"
            >
              <div className="max-w-[85%] rounded-t-2xl rounded-bl-2xl border border-neutral-200 bg-blue-50 px-4 py-2.5">
                <p className="text-sm text-stone-700">
                  Create a Jira ticket for the mobile bug and assign to Sarah
                </p>
              </div>
            </motion.div>
          )}
          {step >= 2 && (
            <motion.div
              key={`tool-${loopKey}`}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2 }}
            >
              <JiraToolCall loopKey={loopKey} />
            </motion.div>
          )}
          {step >= 3 && (
            <motion.div
              key={`a-${loopKey}`}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2 }}
              className="flex items-start gap-2"
            >
              <Icon
                icon="mdi:check-circle"
                className="mt-0.5 shrink-0 text-base text-green-500"
              />
              <p className="text-sm text-stone-700">
                Jira ticket ENG-247 created and assigned to Sarah.
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="border-color-brand shrink-0 border-t px-3 py-2.5">
        <div className="flex items-center justify-between">
          <span className="text-sm text-neutral-400">
            Ask about your notes...
          </span>
          <div className="border-color-brand inline-flex h-7 items-center rounded-lg border px-2.5 text-xs font-medium text-neutral-300">
            <span>Send</span>
          </div>
        </div>
      </div>
    </div>
  );
}
