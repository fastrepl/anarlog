import { Icon } from "@iconify-icon/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import { cn } from "@hypr/utils";

export function SearchToolCall({ loopKey }: { loopKey: number }) {
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    setPhase(0);
    const t1 = setTimeout(() => setPhase(1), 800);
    const t2 = setTimeout(() => setPhase(2), 1400);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [loopKey]);

  const meetings = [
    "Weekly Sync — Oct 12",
    "1:1 with Sarah — Oct 10",
    "Sprint Planning — Oct 8",
  ];

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-neutral-200 bg-white px-3 py-2.5">
      <div className="flex items-center gap-2">
        <div
          className={cn([
            "size-2 rounded-full",
            phase < 2 ? "animate-pulse bg-blue-400" : "bg-blue-400",
          ])}
        />
        <span className="text-xs text-neutral-500">
          {phase < 2 ? "Searching meetings..." : "3 meetings found"}
        </span>
      </div>
      <AnimatePresence>
        {phase >= 1 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="flex flex-col gap-1 overflow-hidden"
          >
            {meetings.slice(0, phase >= 2 ? 3 : 1).map((m) => (
              <motion.div
                key={m}
                initial={{ opacity: 0, x: -8 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ duration: 0.25, ease: "easeOut" }}
                className="flex items-center gap-2 text-xs text-neutral-500"
              >
                <Icon
                  icon="mdi:calendar-outline"
                  className="text-sm text-neutral-400"
                />
                <span>{m}</span>
              </motion.div>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function JiraToolCall({ loopKey }: { loopKey: number }) {
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    setPhase(0);
    const t1 = setTimeout(() => setPhase(1), 600);
    const t2 = setTimeout(() => setPhase(2), 1400);
    const t3 = setTimeout(() => setPhase(3), 2000);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
    };
  }, [loopKey]);

  return (
    <div className="border-color-brand rounded-xl border bg-gradient-to-r from-blue-50 to-stone-50 p-3 shadow-lg">
      <div className="mb-2 flex items-center gap-2 text-xs text-neutral-500">
        <Icon icon="logos:jira" className="text-sm" />
        <AnimatePresence mode="wait">
          {phase < 1 ? (
            <motion.span
              key="creating"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-1.5"
            >
              <span className="inline-block size-3 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-500" />
              Creating ticket...
            </motion.span>
          ) : (
            <motion.span
              key="created"
              initial={{ opacity: 0, x: -4 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center gap-1.5"
            >
              <span>ENG-247</span>
              <span className="rounded-full bg-green-100 px-2 py-0.5 text-[11px] text-green-700">
                Created
              </span>
            </motion.span>
          )}
        </AnimatePresence>
      </div>
      <AnimatePresence>
        {phase >= 2 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="overflow-hidden"
          >
            <p className="text-sm font-medium text-stone-700">
              Mobile UI bug fix
            </p>
          </motion.div>
        )}
      </AnimatePresence>
      <AnimatePresence>
        {phase >= 3 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="mt-2 flex items-center gap-2 overflow-hidden text-xs text-neutral-500"
          >
            <div className="flex size-5 items-center justify-center rounded-full bg-amber-500 text-[10px] text-white">
              S
            </div>
            <span>Sarah</span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function ContactSearchToolCall({
  loopKey,
  static: isStatic,
}: {
  loopKey: number;
  static?: boolean;
}) {
  const [phase, setPhase] = useState(isStatic ? 2 : 0);

  useEffect(() => {
    if (isStatic) return;
    setPhase(0);
    const t1 = setTimeout(() => setPhase(1), 600);
    const t2 = setTimeout(() => setPhase(2), 1200);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [loopKey, isStatic]);

  return (
    <div className="border-color-brand surface rounded-xl border px-3 py-2.5">
      <div className="flex items-center gap-2 text-xs text-neutral-500">
        <div
          className={cn([
            "size-2 rounded-full",
            phase < 2 ? "animate-pulse bg-violet-400" : "bg-violet-400",
          ])}
        />
        <span>{phase < 2 ? "Looking up contacts..." : "Contact found"}</span>
      </div>
      <AnimatePresence>
        {phase >= 1 && (
          <motion.div
            initial={isStatic ? false : { opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="mt-2 flex items-center justify-between overflow-hidden"
          >
            <div className="flex items-center gap-2.5">
              <div className="flex size-7 shrink-0 items-center justify-center rounded-full bg-blue-100 text-xs font-bold text-blue-500">
                S
              </div>
              <div className="flex flex-col">
                <span className="text-xs font-medium text-stone-700">
                  Sarah Chen
                </span>
                <span className="text-[11px] text-neutral-400">
                  Product Lead · Acme Inc
                </span>
              </div>
            </div>
            <span className="text-[11px] text-neutral-400">
              3 meetings together
            </span>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function TranscriptToolCall({
  loopKey,
  static: isStatic,
}: {
  loopKey: number;
  static?: boolean;
}) {
  const [phase, setPhase] = useState(isStatic ? 2 : 0);

  useEffect(() => {
    if (isStatic) return;
    setPhase(0);
    const t1 = setTimeout(() => setPhase(1), 500);
    const t2 = setTimeout(() => setPhase(2), 1200);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
    };
  }, [loopKey, isStatic]);

  return (
    <div className="border-color-brand surface rounded-xl border px-3 py-2.5">
      <div className="flex flex-col gap-2 text-sm">
        <AnimatePresence>
          {phase >= 1 && (
            <motion.div
              initial={isStatic ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, ease: "easeOut" }}
            >
              <span className="text-green-600">Sarah: </span>
              <span className="text-fg">
                The API changes will need at least two sprints...
              </span>
            </motion.div>
          )}
        </AnimatePresence>
        <AnimatePresence>
          {phase >= 2 && (
            <motion.div
              initial={isStatic ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3, ease: "easeOut" }}
            >
              <span className="text-blue-600">Ben: </span>
              <span className="text-fg">
                I can start on the auth module this week.
              </span>
            </motion.div>
          )}
        </AnimatePresence>
        {phase === 0 && (
          <div className="flex items-center gap-2 py-1 text-xs text-neutral-400">
            <span className="inline-block size-3 animate-spin rounded-full border-2 border-neutral-200 border-t-neutral-400" />
            Reading transcript...
          </div>
        )}
      </div>
    </div>
  );
}
