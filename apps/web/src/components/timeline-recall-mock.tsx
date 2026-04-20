import { motion } from "motion/react";

const timelineBlocks = [
  {
    time: "10:00 – 11:30 AM",
    title: "Finances & Admin",
    body: "Checked Mercury banking and PostHog analytics, then shifted to tax and payroll admin.",
    bullets: [
      "Pulled contractor documents from Gusto",
      "Drafted W-9 request emails from Mail",
    ],
  },
  {
    time: "11:50 AM – 12:25 PM",
    title: 'Writing "Moats Suck"',
    body: "Drafted an essay in Obsidian, then cross-published to LinkedIn and X.",
    bullets: ["~600–780 words", "Shared in #general and #uchar"],
  },
  {
    time: "1:00 – 3:30 PM",
    title: "Coding — Char v0.0.8",
    body: "Updated Char and configured transcription settings.",
    bullets: [
      "Added created_at timestamp display",
      "Explored Soniqo Docs for speech enhancement",
    ],
  },
  {
    time: "3:30 – 4:30 PM",
    title: "Discord & Team",
    body: "Active in the CEO's Office channel, directing the AI agent.",
    bullets: ["Reviewed SQLite migration work", "Triaged GitHub issue #5047"],
  },
];

export function TimelineRecallMock() {
  return (
    <div className="border-color-brand surface relative h-[420px] w-full overflow-hidden rounded-xl border shadow-xl">
      <div className="pointer-events-none absolute inset-x-0 top-0 z-10 h-12 bg-gradient-to-b from-white to-transparent" />
      <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 h-12 bg-gradient-to-t from-white to-transparent" />

      <motion.div
        animate={{ y: ["0%", "-50%"] }}
        transition={{ duration: 30, ease: "linear", repeat: Infinity }}
        className="flex flex-col"
      >
        {[...timelineBlocks, ...timelineBlocks].map((block, i) => (
          <TimelineBlock key={i} {...block} />
        ))}
      </motion.div>
    </div>
  );
}

function TimelineBlock({
  time,
  title,
  body,
  bullets,
}: {
  time: string;
  title: string;
  body: string;
  bullets: string[];
}) {
  return (
    <div className="border-color-brand flex flex-col gap-2 border-b px-6 py-5">
      <div className="font-mono text-xs tracking-wider text-neutral-500 uppercase">
        {time}
      </div>
      <h4 className="text-color text-base font-semibold">{title}</h4>
      <p className="text-color-secondary text-sm leading-relaxed">{body}</p>
      <ul className="text-color-secondary flex flex-col gap-1 text-sm">
        {bullets.map((b) => (
          <li key={b} className="flex gap-2">
            <span className="text-neutral-400">•</span>
            <span>{b}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
