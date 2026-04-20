import { Icon } from "@iconify-icon/react";
import { AnimatePresence, motion, useInView } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { JiraToolCall } from "./ai-feature-panel";

export function JiraWorkflowMock() {
  const ref = useRef<HTMLDivElement>(null);
  const isInView = useInView(ref, { amount: 0.3 });
  const [step, setStep] = useState(0);

  useEffect(() => {
    if (!isInView) {
      return;
    }

    setStep(0);
    const t1 = setTimeout(() => setStep(1), 400);
    const t2 = setTimeout(() => setStep(2), 1200);
    const t3 = setTimeout(() => setStep(3), 3800);
    const t4 = setTimeout(() => setStep(4), 4600);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
      clearTimeout(t4);
    };
  }, [isInView]);

  return (
    <div ref={ref} className="flex w-full flex-col overflow-hidden">
      <div className="flex min-h-[340px] flex-col justify-end gap-3">
        <AnimatePresence mode="popLayout">
          {step >= 1 && (
            <motion.div
              key="q"
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
              key="tool"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.2 }}
            >
              <JiraToolCall loopKey={0} />
            </motion.div>
          )}
          {step >= 3 && (
            <motion.div
              key="a"
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
          {step >= 4 && (
            <motion.div
              key="ai"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.25, ease: "easeOut" }}
              className="flex w-full justify-start"
            >
              <div className="max-w-[85%] rounded-t-xl rounded-br-xl border border-stone-200 bg-gradient-to-b from-white to-stone-100 px-4 py-3">
                <p className="text-sm text-stone-700">
                  Done — I pulled the bug context from today's standup notes,
                  set priority to High, and tagged the mobile-app label so
                  Sarah's team sees it in their next triage.
                </p>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="border-color-brand mt-4 shrink-0 rounded-xl border p-4">
        <div className="flex items-center justify-between">
          <span className="text-sm text-neutral-400">
            Ask Charlie anything...
          </span>
          <div className="border-color-brand text-color-secondary inline-flex h-7 items-center rounded-lg border px-2.5 text-xs font-medium">
            <span>Send</span>
          </div>
        </div>
      </div>
    </div>
  );
}
