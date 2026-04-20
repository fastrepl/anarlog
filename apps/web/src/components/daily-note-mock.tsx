import { Icon } from "@iconify-icon/react";
import { AnimatePresence, motion } from "motion/react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { NoteTab } from "@hypr/ui/components/ui/note-tab";
import { cn } from "@hypr/utils";

const PROFILE_MENU_ITEMS: {
  icon: string;
  label: string;
  shortcut?: string[];
}[] = [
  {
    icon: "mdi:account-multiple-outline",
    label: "Contacts",
    shortcut: ["⌘", "⇧", "O"],
  },
  {
    icon: "mdi:calendar-blank-outline",
    label: "Calendar",
    shortcut: ["⌘", "⇧", "C"],
  },
  {
    icon: "mdi:cog-outline",
    label: "Settings",
    shortcut: ["⌘", ","],
  },
  {
    icon: "mdi:help-circle-outline",
    label: "Help",
  },
];

const EDITOR_TABS = ["Summary", "Memos", "Transcript"] as const;

type MeetingContent = {
  title: string;
  time: string;
  summary: React.ReactNode;
  memos: string;
  transcript: { speaker: string; text: string }[];
};

const MEETINGS: Record<string, MeetingContent> = {
  "team-standup": {
    title: "Team Standup",
    time: "9:30 AM",
    summary: (
      <>
        <h4>Updates</h4>
        <ul>
          <li>
            Mobile navigation prototype at 80% — Sarah blocked on the final icon
            set
          </li>
          <li>Auth middleware fix shipped yesterday, no rollbacks</li>
          <li>Victor's dashboard v2 ready for review today</li>
        </ul>
        <h4>Blockers</h4>
        <ul>
          <li>Mobile build pipeline still flaky on PR branches</li>
        </ul>
        <h4>Action Items</h4>
        <ul>
          <li>Ben to finish auth SDK docs by Friday</li>
          <li>Alice to unblock mobile build before EOD</li>
        </ul>
      </>
    ),
    memos:
      "standup 9:30\nmob nav 80%\nauth shipped - clean\nvictor dash v2 ready\nblocker: mobile build flaky\nben SDK docs Friday",
    transcript: [
      { speaker: "You", text: "Let's make this quick. Sarah — mobile nav?" },
      {
        speaker: "Sarah",
        text: "Prototype's at about 80%. Blocked on the final icon set from the design system.",
      },
      { speaker: "You", text: "Ben, can you get her unblocked today?" },
      {
        speaker: "Ben",
        text: "Yeah. I'll drop them in the channel within an hour.",
      },
      {
        speaker: "Victor",
        text: "Dashboard v2 is ready for review — can I send it around now?",
      },
      { speaker: "You", text: "Please. I'll look this afternoon." },
    ],
  },
  "design-review-w-sarah": {
    title: "Design review w/ Sarah",
    time: "11:00 AM",
    summary: (
      <>
        <h4>Onboarding redesign</h4>
        <ul>
          <li>Cut the workspace setup step entirely — A/B showed a 12% lift</li>
          <li>
            Tighten the first-run tour to three tooltips, skip on the second
            visit
          </li>
        </ul>
        <h4>Open Questions</h4>
        <ul>
          <li>Progress indicator, or let users jump around?</li>
          <li>Does the mobile waitlist flow share the same layout?</li>
        </ul>
        <h4>Next Steps</h4>
        <ul>
          <li>Sarah to ship final mocks by Thursday</li>
          <li>Engineering spike on the waitlist integration — 2 days</li>
        </ul>
      </>
    ),
    memos:
      "onboarding redesign\ncut workspace step — 12% lift\nfirst-run tour: 3 tooltips\nopen: progress indicator?\nsarah mocks Thurs\nwaitlist spike 2d",
    transcript: [
      {
        speaker: "Sarah",
        text: "I want to cut the workspace setup step completely. The A/B showed a clear 12% lift.",
      },
      { speaker: "You", text: "Ship it. What else?" },
      {
        speaker: "Sarah",
        text: "First-run tour — three tooltips max, skip on revisit.",
      },
      { speaker: "You", text: "Agreed. What's still open?" },
      {
        speaker: "Sarah",
        text: "Progress indicator vs free navigation — I'm split. And the mobile layout for the waitlist.",
      },
      {
        speaker: "You",
        text: "Let's ship without progress for now, measure, iterate.",
      },
    ],
  },
  "customer-call-acme": {
    title: "Customer call: Acme",
    time: "1:00 PM",
    summary: (
      <>
        <h4>Feedback</h4>
        <ul>
          <li>
            Using summaries as prep for their own customer calls — big time save
          </li>
          <li>Want direct Notion export of summaries</li>
        </ul>
        <h4>Feature Requests</h4>
        <ul>
          <li>Bulk export — all meetings from a given week</li>
          <li>
            Keyword alerts (e.g. notify on &ldquo;contract&rdquo; mentions)
          </li>
        </ul>
        <h4>Follow-ups</h4>
        <ul>
          <li>Send roadmap doc to Acme by Friday</li>
          <li>Intro Dana to Maya for Notion integration scoping</li>
        </ul>
      </>
    ),
    memos:
      "Acme loves summaries\nasks: notion export, bulk export, keyword alerts\nsend roadmap Friday\nintro Dana → Maya",
    transcript: [
      {
        speaker: "Dana (Acme)",
        text: "We're using the summaries as prep for our own customer calls. Huge time save.",
      },
      { speaker: "You", text: "Glad to hear. Anything missing?" },
      {
        speaker: "Dana (Acme)",
        text: "Notion export, directly into our ops workspace. Ideally bulk — a week's worth.",
      },
      {
        speaker: "You",
        text: "We can scope the Notion integration. Keyword alerts?",
      },
      {
        speaker: "Dana (Acme)",
        text: "Yes, especially anything mentioning contracts.",
      },
      { speaker: "You", text: "I'll intro you to Maya this week for scoping." },
    ],
  },
  "1-1-with-alice": {
    title: "1:1 with Alice",
    time: "3:00 PM",
    summary: (
      <>
        <h4>Current Work</h4>
        <ul>
          <li>Mobile build pipeline fix — shipping by EOD</li>
          <li>Dashboard v2 review scheduled with Victor tomorrow</li>
        </ul>
        <h4>Career &amp; Growth</h4>
        <ul>
          <li>
            Interested in owning the platform architecture track next quarter
          </li>
          <li>Wants to attend React Summit in June — budget ~$2.4k</li>
        </ul>
        <h4>Follow-ups</h4>
        <ul>
          <li>Approve conference budget by Friday</li>
          <li>Schedule architecture review series for next month</li>
        </ul>
      </>
    ),
    memos:
      "alice 1:1\nmobile build fix EOD\ndash v2 review tmrw\ngrowth: platform arch ownership\nreact summit — $2.4k\napprove conf by Fri",
    transcript: [
      { speaker: "You", text: "How's the mobile build pipeline fix going?" },
      {
        speaker: "Alice",
        text: "Found the root cause — flaky test in the auth module. Fix is small, shipping by EOD.",
      },
      { speaker: "You", text: "Good. What's on your mind career-wise?" },
      {
        speaker: "Alice",
        text: "I'd like to take ownership of the platform architecture track next quarter.",
      },
      { speaker: "You", text: "Let's make that happen. Conferences?" },
      {
        speaker: "Alice",
        text: "React Summit in June. Around $2.4k including travel.",
      },
      { speaker: "You", text: "Send me the request, I'll approve by Friday." },
    ],
  },
  "investor-update": {
    title: "Investor update",
    time: "4:30 PM",
    summary: (
      <>
        <h4>Metrics</h4>
        <ul>
          <li>MRR up 18% MoM, self-serve now 40% of new signups</li>
          <li>p95 latency down from 340ms to 195ms</li>
          <li>NPS 54 (+6 vs last quarter)</li>
        </ul>
        <h4>Runway</h4>
        <ul>
          <li>18 months at current burn</li>
          <li>Considering a bridge round at end of Q3</li>
        </ul>
        <h4>Next Quarter</h4>
        <ul>
          <li>Ship integrations marketplace v1</li>
          <li>Open mobile beta to 500 users</li>
          <li>Hire senior platform engineer</li>
        </ul>
      </>
    ),
    memos:
      "investor update\nMRR +18% MoM\nself-serve = 40% new\np95: 340 → 195ms\nNPS 54 (+6)\nrunway 18mo\nmaybe bridge end Q3\nnext Q: marketplace v1, mobile beta 500, senior platform hire",
    transcript: [
      {
        speaker: "You",
        text: "MRR up 18% month over month. Self-serve is 40% of new signups.",
      },
      { speaker: "Investor", text: "Encouraging. Latency?" },
      {
        speaker: "You",
        text: "p95 from 340 to 195. The platform work is paying off.",
      },
      { speaker: "Investor", text: "Runway?" },
      {
        speaker: "You",
        text: "18 months at current burn. May do a bridge at end of Q3 to accelerate marketplace work.",
      },
      { speaker: "Investor", text: "Happy to reconnect then. Next quarter?" },
      {
        speaker: "You",
        text: "Marketplace v1, mobile beta to 500 users, senior platform hire.",
      },
    ],
  },
  "onboarding-new-hire": {
    title: "Onboarding: new hire",
    time: "10:30 AM",
    summary: (
      <>
        <h4>Today</h4>
        <ul>
          <li>Team intros — 1:1 with every engineer</li>
          <li>Tooling: laptop, GitHub access, 1Password vault</li>
          <li>Codebase architecture walkthrough</li>
        </ul>
        <h4>First Week</h4>
        <ul>
          <li>Ship a small PR — docs or a tiny bug fix</li>
          <li>Attend all squad syncs</li>
        </ul>
        <h4>First Month</h4>
        <ul>
          <li>Own a medium-scope project end to end</li>
          <li>Present learnings at end-of-month retro</li>
        </ul>
      </>
    ),
    memos:
      "new hire onboarding\nintros, tooling, arch tour\nweek 1: small PR, squad syncs\nmonth 1: med project, retro presentation",
    transcript: [
      {
        speaker: "You",
        text: "Welcome. Today is introductions and getting your tooling sorted.",
      },
      {
        speaker: "New hire",
        text: "Excited to be here. What's the expectation for the first week?",
      },
      {
        speaker: "You",
        text: "Ship a small PR — docs or a bug fix. Meet every engineer 1:1. Attend all squad syncs.",
      },
      { speaker: "New hire", text: "And longer term?" },
      {
        speaker: "You",
        text: "By end of month, own a medium-scope project and present learnings at the retro.",
      },
    ],
  },
};

type TabEntry = { id: string; title: string };
type ActiveTab = "home" | string;

const OpenMeetingContext = createContext<((title: string) => void) | null>(
  null,
);

function useOpenMeeting() {
  return useContext(OpenMeetingContext);
}

function slugify(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

export function DailyNoteMock({ className }: { className?: string }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const tomorrowRef = useRef<HTMLElement>(null);
  const todayRef = useRef<HTMLElement>(null);
  const yesterdayRef = useRef<HTMLElement>(null);

  const [openTabs, setOpenTabs] = useState<TabEntry[]>([]);
  const [activeTab, setActiveTab] = useState<ActiveTab>("home");
  const [profileOpen, setProfileOpen] = useState(false);
  const profileContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!profileOpen) return;
    function handleClickOutside(e: MouseEvent) {
      if (
        profileContainerRef.current &&
        !profileContainerRef.current.contains(e.target as Node)
      ) {
        setProfileOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [profileOpen]);

  const { tomorrow, today, yesterday } = useMemo(() => {
    const now = new Date();
    return {
      tomorrow: formatDayLabel(addDays(now, 1)),
      today: formatDayLabel(now),
      yesterday: formatDayLabel(addDays(now, -1)),
    };
  }, []);

  useLayoutEffect(() => {
    const scroll = scrollRef.current;
    const section = todayRef.current;
    if (!scroll || !section) return;
    scroll.scrollTop = section.offsetTop;
  }, []);

  const scrollTo = (ref: React.RefObject<HTMLElement | null>) => () => {
    ref.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const openMeeting = useCallback((title: string) => {
    const id = slugify(title);
    setOpenTabs((prev) =>
      prev.some((t) => t.id === id) ? prev : [...prev, { id, title }],
    );
    setActiveTab(id);
  }, []);

  const closeTab = useCallback((id: string) => {
    setOpenTabs((prev) => prev.filter((t) => t.id !== id));
    setActiveTab((current) => (current === id ? "home" : current));
  }, []);

  return (
    <OpenMeetingContext.Provider value={openMeeting}>
      <div
        className={cn([
          "border-color-brand flex flex-col overflow-hidden rounded-2xl border bg-stone-100 px-2 pb-2 shadow-xl transition-shadow duration-200 hover:shadow-2xl",
          className,
        ])}
      >
        <div className="flex h-11 shrink-0 items-center gap-2 pl-2">
          <div className="flex gap-2">
            <div className="size-3 rounded-full bg-red-400" />
            <div className="size-3 rounded-full bg-yellow-400" />
            <div className="size-3 rounded-full bg-green-400" />
          </div>

          <button
            type="button"
            onClick={() => setActiveTab("home")}
            className={cn([
              "ml-2 flex size-7 items-center justify-center rounded-md transition-colors",
              activeTab === "home"
                ? "border border-stone-400 bg-neutral-200/50 text-neutral-900"
                : "text-neutral-700 hover:bg-neutral-100",
            ])}
          >
            <Icon icon="mdi:home-variant-outline" className="text-base" />
          </button>

          <div className="scrollbar-hide flex min-w-0 items-center gap-1 overflow-x-auto">
            {openTabs.map((tab) => (
              <MeetingTab
                key={tab.id}
                title={tab.title}
                active={activeTab === tab.id}
                onSelect={() => setActiveTab(tab.id)}
                onClose={() => closeTab(tab.id)}
              />
            ))}
          </div>

          <div className="flex-1" />

          <div className="flex items-center gap-1">
            <button className="flex size-7 items-center justify-center text-neutral-500 hover:text-neutral-700">
              <Icon icon="mdi:magnify" className="text-base" />
            </button>
            <button className="flex size-7 items-center justify-center text-neutral-500 hover:text-neutral-700">
              <Icon icon="mdi:plus" className="text-base" />
            </button>
            <div ref={profileContainerRef} className="relative">
              <button
                type="button"
                onClick={() => setProfileOpen((v) => !v)}
                className={cn([
                  "flex size-7 cursor-pointer items-center justify-center overflow-hidden rounded-full bg-amber-100 transition-all",
                  profileOpen && "ring-2 ring-stone-400 ring-offset-1",
                ])}
              >
                <span className="text-[10px] font-medium text-amber-800">
                  JD
                </span>
              </button>

              <AnimatePresence>
                {profileOpen && (
                  <motion.div
                    initial={{ opacity: 0, y: -6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.15, ease: "easeOut" }}
                    className="border-border-subtle absolute top-full right-0 z-20 mt-1 w-[260px] overflow-hidden rounded-xl border bg-white py-1 shadow-lg"
                  >
                    {PROFILE_MENU_ITEMS.map((item) => (
                      <div key={item.label} className="px-1">
                        <div className="flex w-full items-center justify-between gap-2 rounded-lg px-2.5 py-1.5 text-sm text-neutral-800 transition-colors hover:bg-neutral-100">
                          <div className="flex items-center gap-2.5">
                            <Icon
                              icon={item.icon}
                              className="shrink-0 text-base text-neutral-600"
                            />
                            {item.label}
                          </div>
                          {item.shortcut && (
                            <div className="flex items-center gap-0.5">
                              {item.shortcut.map((key, i) => (
                                <kbd
                                  key={i}
                                  className="flex size-5 items-center justify-center rounded border border-neutral-200 bg-neutral-50 font-sans text-[10px] text-neutral-500"
                                >
                                  {key}
                                </kbd>
                              ))}
                            </div>
                          )}
                        </div>
                      </div>
                    ))}

                    <div className="border-border-subtle my-1 border-t" />

                    <div className="px-1">
                      <div className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm transition-colors hover:bg-neutral-100">
                        <div className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-full bg-amber-100">
                          <span className="text-[10px] font-medium text-amber-800">
                            JD
                          </span>
                        </div>
                        <span className="min-w-0 flex-1 truncate text-neutral-700">
                          jane@hyprnote.com
                        </span>
                        <span className="shrink-0 rounded-md bg-blue-50 px-1.5 py-0.5 text-[10px] font-semibold tracking-wider text-blue-600">
                          LITE
                        </span>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>

        {activeTab !== "home" ? (
          <MeetingView slug={activeTab} />
        ) : (
          <div
            ref={scrollRef}
            className="scrollbar-hide surface border-color-brand min-h-0 flex-1 snap-y snap-proximity overflow-y-auto rounded-lg border"
          >
            <section
              ref={tomorrowRef}
              className="flex shrink-0 snap-start flex-col"
            >
              <DayHeader
                label={tomorrow}
                muted
                onSelect={scrollTo(tomorrowRef)}
              />
            </section>

            <section
              ref={todayRef}
              className="border-color-subtle flex min-h-full snap-start flex-col border-b"
            >
              <DayHeader label={today} today onSelect={scrollTo(todayRef)} />
              <TaskList>
                <MeetingTaskRow title="Team Standup" time="9:30 AM" done />
                <MeetingTaskRow
                  title="Design review w/ Sarah"
                  time="11:00 AM"
                />
                <TextNote>
                  Found new onboarding instrument, need to check API coverage
                  before Friday.
                </TextNote>
                <BulletList
                  items={[
                    "Sketch 2 onboarding variants for the waitlist flow",
                    "Map required signup fields against current Stripe payload",
                    "Draft trigger schedule with Eva",
                  ]}
                />
                <MeetingTaskRow title="Customer call: Acme" time="1:00 PM" />
                <ActionTaskRow label="Finish mobile navigation prototype" due />
                <TextNote>
                  Idea: weekly digest email from Linear activity — ship as
                  Friday internal test first.
                </TextNote>
                <BulletList
                  items={[
                    "Pull closed issues from the last 7 days",
                    "Group by project + owner",
                    "Send Friday 4pm via the existing mailer",
                  ]}
                />
                <MeetingTaskRow title="1:1 with Alice" time="3:00 PM" />
                <ActionTaskRow
                  label="Review dashboard mockups from Victor"
                  due
                />
                <MeetingTaskRow title="Investor update" time="4:30 PM" />
                <ActionTaskRow label="Respond to vendor contract from Legal" />
              </TaskList>
            </section>

            <section
              ref={yesterdayRef}
              className="flex min-h-full snap-start flex-col"
            >
              <DayHeader
                label={yesterday}
                muted
                onSelect={scrollTo(yesterdayRef)}
              />
              <TaskList>
                <MeetingTaskRow title="Team Standup" time="9:30 AM" done />
                <MeetingTaskRow
                  title="Onboarding: new hire"
                  time="10:30 AM"
                  done
                />
                <TextNote>
                  Shipped auth middleware fix before lunch — no rollbacks,
                  metrics look clean.
                </TextNote>
                <ActionTaskRow label="Ship auth middleware fix" done />
                <ActionTaskRow label="Draft release notes for v2.3" done />
                <MeetingTaskRow title="Investor update" time="4:00 PM" done />
                <ActionTaskRow label="Archive last quarter's OKRs" />
              </TaskList>
            </section>
          </div>
        )}
      </div>
    </OpenMeetingContext.Provider>
  );
}

function MeetingView({ slug }: { slug: string }) {
  const meeting = MEETINGS[slug];
  const [activeEditorTab, setActiveEditorTab] = useState(0);

  if (!meeting) {
    return (
      <div className="surface border-color-brand flex min-h-0 flex-1 items-center justify-center rounded-lg border p-8 text-sm text-neutral-500">
        No content for this meeting.
      </div>
    );
  }

  return (
    <div className="surface border-color-brand flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border">
      <div className="flex items-center justify-between gap-2 px-4 pt-3 pb-1">
        <div className="flex min-w-0 items-center gap-1 text-xs text-neutral-600">
          <span className="shrink-0 text-neutral-500">Daily Note</span>
          <Icon
            icon="mdi:chevron-right"
            className="shrink-0 text-xs text-neutral-400"
          />
          <span className="truncate text-neutral-700">{meeting.title}</span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button className="inline-flex h-6 items-center gap-1 rounded px-1.5 text-xs text-neutral-600">
            <Icon
              icon="mdi:calendar-blank-outline"
              className="shrink-0 text-xs"
            />
            <span>{meeting.time}</span>
          </button>
          <button className="flex size-6 items-center justify-center text-neutral-600">
            <Icon icon="mdi:dots-horizontal" className="text-sm" />
          </button>
        </div>
      </div>

      <div className="flex items-center gap-2 px-4 pt-2">
        <h3 className="flex-1 truncate text-xl font-semibold text-neutral-900">
          {meeting.title}
        </h3>
        <Icon
          icon="mdi:creation"
          className="shrink-0 text-sm text-neutral-400"
        />
      </div>

      <div className="px-3 pt-1">
        <div className="flex items-center gap-1">
          {EDITOR_TABS.map((label, i) => (
            <NoteTab
              key={label}
              isActive={activeEditorTab === i}
              onClick={() => setActiveEditorTab(i)}
            >
              {label}
            </NoteTab>
          ))}
        </div>
      </div>

      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto px-4 pt-1 pb-6">
        {activeEditorTab === 0 && (
          <div className="mock-summary text-sm leading-relaxed text-neutral-700">
            {meeting.summary}
          </div>
        )}
        {activeEditorTab === 1 && (
          <div className="font-mono text-sm leading-relaxed whitespace-pre-wrap text-neutral-700">
            {meeting.memos}
          </div>
        )}
        {activeEditorTab === 2 && (
          <div className="flex flex-col gap-3">
            {meeting.transcript.map((line, i) => (
              <div key={i} className="flex gap-3">
                <span className="w-16 shrink-0 pt-0.5 text-right text-xs font-medium text-neutral-400">
                  {line.speaker}
                </span>
                <span className="flex-1 text-sm leading-relaxed text-neutral-700">
                  {line.text}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function MeetingTab({
  title,
  active,
  onSelect,
  onClose,
}: {
  title: string;
  active: boolean;
  onSelect: () => void;
  onClose: () => void;
}) {
  return (
    <div
      role="button"
      onClick={onSelect}
      className={cn([
        "flex h-7 w-[160px] shrink-0 cursor-pointer items-center gap-1.5 rounded-lg border px-2 text-xs transition-colors",
        active
          ? "border-stone-400 bg-neutral-200/50 text-neutral-900"
          : "border-transparent text-neutral-500 hover:bg-neutral-100",
      ])}
    >
      <Icon icon="mdi:note-text-outline" className="shrink-0 text-xs" />
      <span className="min-w-0 flex-1 truncate">{title}</span>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        className="flex size-4 shrink-0 items-center justify-center rounded hover:bg-neutral-300/60"
      >
        <Icon icon="mdi:close" className="text-xs text-neutral-700" />
      </button>
    </div>
  );
}

function TaskList({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-col gap-1.5 px-8 pt-2 pb-6">{children}</div>;
}

function DayHeader({
  label,
  today,
  muted,
  onSelect,
}: {
  label: string;
  today?: boolean;
  muted?: boolean;
  onSelect?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className="surface sticky top-0 z-10 flex w-full shrink-0 items-center gap-2 px-8 py-4 text-left"
    >
      <h4
        className={cn([
          "text-xl",
          muted
            ? "font-medium text-neutral-400"
            : "font-semibold text-neutral-900",
        ])}
      >
        {label}
      </h4>
      {today && (
        <span className="rounded-full bg-neutral-900 px-2 py-0.5 text-xs font-medium text-white">
          today
        </span>
      )}
    </button>
  );
}

function MeetingTaskRow({
  title,
  time,
  done,
}: {
  title: string;
  time: string;
  done?: boolean;
}) {
  const openMeeting = useOpenMeeting();
  return (
    <div
      role="button"
      onClick={() => openMeeting?.(title)}
      className="group flex cursor-pointer items-center gap-3 rounded-md py-1.5 pr-3 hover:bg-neutral-100/70"
    >
      <TaskCheckbox done={done} />
      <Icon
        icon="mdi:calendar-blank-outline"
        className="shrink-0 text-base text-neutral-400"
      />
      <span
        className={cn([
          "flex-1 text-sm",
          done ? "text-neutral-400 line-through" : "text-neutral-800",
        ])}
      >
        {title}
      </span>
      <span className="shrink-0 font-mono text-xs text-neutral-400">
        {time}
      </span>
    </div>
  );
}

function TextNote({ children }: { children: React.ReactNode }) {
  return (
    <p className="py-1 text-sm leading-relaxed text-neutral-700">{children}</p>
  );
}

function BulletList({ items }: { items: string[] }) {
  return (
    <ul className="flex flex-col gap-1 py-1">
      {items.map((item, i) => (
        <li
          key={i}
          className="flex items-start gap-3 text-sm leading-relaxed text-neutral-700"
        >
          <span className="mt-[9px] size-1 shrink-0 rounded-full bg-neutral-400" />
          <span>{item}</span>
        </li>
      ))}
    </ul>
  );
}

function ActionTaskRow({
  label,
  due,
  done,
}: {
  label: string;
  due?: boolean;
  done?: boolean;
}) {
  return (
    <div className="flex items-center gap-3 py-1.5 pr-3">
      <TaskCheckbox done={done} />
      <span
        className={cn([
          "flex-1 text-sm",
          done ? "text-neutral-400 line-through" : "text-neutral-800",
        ])}
      >
        {label}
      </span>
      {due && (
        <span className="shrink-0 rounded-full border border-neutral-200 px-2.5 py-0.5 text-[11px] font-medium text-neutral-500">
          Due
        </span>
      )}
    </div>
  );
}

function TaskCheckbox({ done }: { done?: boolean }) {
  return (
    <span
      className={cn([
        "flex size-4 shrink-0 items-center justify-center rounded border",
        done
          ? "border-neutral-900 bg-neutral-900 text-white"
          : "border-neutral-300 bg-white",
      ])}
    >
      {done && <Icon icon="mdi:check" className="text-[10px]" />}
    </span>
  );
}

function addDays(date: Date, days: number): Date {
  const copy = new Date(date);
  copy.setDate(copy.getDate() + days);
  return copy;
}

function formatDayLabel(date: Date): string {
  const month = date.toLocaleString("en-US", { month: "long" });
  const day = date.getDate();
  return `${month} ${day}${ordinalSuffix(day)}`;
}

function ordinalSuffix(n: number): string {
  const tens = n % 100;
  if (tens >= 11 && tens <= 13) return "th";
  switch (n % 10) {
    case 1:
      return "st";
    case 2:
      return "nd";
    case 3:
      return "rd";
    default:
      return "th";
  }
}
