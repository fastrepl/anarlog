import { Icon } from "@iconify-icon/react";
import {
  ArrowLeftIcon,
  CalendarIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ChevronUpIcon,
  CircleHelpIcon,
  EllipsisIcon,
  FolderOpenIcon,
  MailIcon,
  MessageCircleIcon,
  PanelRightIcon,
  PlusIcon,
  SearchIcon,
  SettingsIcon,
  SlidersHorizontalIcon,
  SparklesIcon,
  SquareArrowOutUpRightIcon,
  UsersIcon,
  X,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";

import { NoteTab } from "@hypr/ui/components/ui/note-tab";
import { cn } from "@hypr/utils";

import { CharLogo } from "./sidebar";

type MockNote = {
  id: string;
  time: string;
  title: string;
  group: "Today" | "Yesterday";
  noteContent: {
    title: string;
    summary: React.ReactNode;
    memos: string;
    transcript: { speaker: string; text: string }[];
  };
};

const mockNotes: MockNote[] = [
  {
    id: "n1",
    time: "9:30 AM",
    title: "Team Standup",
    group: "Today",
    noteContent: {
      title: "Team Standup — March 20",
      summary: (
        <>
          <h4>Mobile UI Update and API Adjustments</h4>
          <ul>
            <li>
              Sarah presented the new mobile UI update, which includes a
              streamlined navigation bar and improved button placements for
              better accessibility.
            </li>
            <li>
              Ben confirmed that API adjustments are needed to support dynamic
              UI changes, particularly for fetching personalized user data more
              efficiently.
            </li>
            <li>
              The UI update will be implemented in phases, starting with core
              navigation improvements. Ben will ensure API modifications are
              completed before development begins.
            </li>
          </ul>
          <h4>New Dashboard — Urgent Priority</h4>
          <ul>
            <li>
              Alice emphasized that the new analytics dashboard must be
              prioritized due to increasing stakeholder demand.
            </li>
            <li>
              The new dashboard will feature real-time user engagement metrics
              and a customizable reporting system.
            </li>
          </ul>
        </>
      ),
      memos: "ui update - moble\napi\nnew dash - urgnet",
      transcript: [
        {
          speaker: "You",
          text: "Alright, let's go through the updates. Sarah, how's the mobile UI coming along?",
        },
        {
          speaker: "Sarah",
          text: "We've got the new navigation bar ready. Button placements are improved for accessibility — much cleaner flow overall.",
        },
        {
          speaker: "Ben",
          text: "I'll need to adjust the API to support the dynamic UI changes. Mainly around fetching personalized user data more efficiently.",
        },
        {
          speaker: "Alice",
          text: "Before we move on — the new analytics dashboard needs to be prioritized. Stakeholders have been asking about it a lot.",
        },
        {
          speaker: "You",
          text: "Agreed. Let's phase the UI update and make sure the API work is done first. Alice, can you spec out the dashboard requirements by Friday?",
        },
      ],
    },
  },
  {
    id: "n2",
    time: "2:00 PM",
    title: "Product Review",
    group: "Today",
    noteContent: {
      title: "Product Review — Sprint 14",
      summary: (
        <>
          <h4>Overview</h4>
          <p>
            Reviewed the new onboarding flow with the design team. A/B test
            results show a 12% conversion lift on the simplified variant.
          </p>
          <h4>Key Decisions</h4>
          <ul>
            <li>Ship shortened onboarding flow to 100% of users next week</li>
            <li>Defer fuzzy search to next quarter — needs more scoping</li>
            <li>
              Prioritize mobile responsiveness fixes for the settings page
            </li>
          </ul>
          <h4>Open Questions</h4>
          <ul>
            <li>Should we add a progress indicator to the new onboarding?</li>
            <li>What's the performance budget for the search improvements?</li>
          </ul>
        </>
      ),
      memos:
        "The 12% lift is solid — worth noting that most of the gain came from removing the workspace setup step. Users were dropping off there.\n\nFuzzy search is interesting but we don't have a clear spec yet. Alex volunteered to write one.",
      transcript: [
        {
          speaker: "You",
          text: "Let's start with the onboarding A/B results. Alex, can you walk us through?",
        },
        {
          speaker: "Alex",
          text: "Sure. The simplified variant — the one without the workspace setup step — showed a 12% lift in completion rate.",
        },
        {
          speaker: "Maya",
          text: "That's significant. I think we should ship it to everyone.",
        },
        {
          speaker: "You",
          text: "Agreed. Let's plan for a full rollout next week. What about the search improvements?",
        },
        {
          speaker: "Alex",
          text: "The fuzzy matching feature needs more scoping. I'd suggest we push it to next quarter.",
        },
      ],
    },
  },
  {
    id: "n3",
    time: "11:00 AM",
    title: "1:1 with Sarah",
    group: "Today",
    noteContent: {
      title: "1:1 with Sarah",
      summary: (
        <>
          <h4>Career Growth</h4>
          <ul>
            <li>Sarah wants to lead the API documentation project</li>
            <li>
              Interested in more cross-team visibility — will join platform
              architecture meetings starting next month
            </li>
          </ul>
          <h4>Current Work</h4>
          <ul>
            <li>Mobile notification redesign shipped and performing well</li>
            <li>
              Next focus: accessibility audit for the dashboard components
            </li>
          </ul>
          <h4>Follow-ups</h4>
          <ul>
            <li>Approve conference budget request by Friday</li>
            <li>Send invite for architecture meeting series</li>
          </ul>
        </>
      ),
      memos:
        "Sarah is clearly ready for more ownership — the API docs project is a good fit. Should also think about putting her name forward for the tech lead track.\n\nConference is React Summit in June. Budget is ~$2,400 including travel.",
      transcript: [
        {
          speaker: "You",
          text: "How's everything going? How did the notification redesign land?",
        },
        {
          speaker: "Sarah",
          text: "Really well! TestFlight feedback has been positive. I'm feeling good about it.",
        },
        {
          speaker: "You",
          text: "Great. Let's talk about what's next for you. Any projects you're excited about?",
        },
        {
          speaker: "Sarah",
          text: "I'd love to take point on the API documentation project. I've been thinking about how to structure it.",
        },
        {
          speaker: "You",
          text: "I think that's a great fit. Let's make it happen.",
        },
      ],
    },
  },
  {
    id: "n4",
    time: "4:00 PM",
    title: "Q1 Planning Sync",
    group: "Yesterday",
    noteContent: {
      title: "Q1 Planning Sync",
      summary: (
        <>
          <h4>Q1 Priorities</h4>
          <ul>
            <li>Launch self-serve tier — targeting end of February</li>
            <li>Reduce p95 API latency below 200ms</li>
            <li>Ship integrations marketplace v1</li>
          </ul>
          <h4>Constraints</h4>
          <ul>
            <li>
              Engineering headcount stays flat — no new hires this quarter
            </li>
            <li>Need to be disciplined about scope to hit all three goals</li>
          </ul>
          <h4>Marketing</h4>
          <ul>
            <li>Public launch event planned for late March</li>
            <li>Blog series on the self-serve tier starts mid-February</li>
          </ul>
        </>
      ),
      memos:
        "Three priorities is already ambitious. Self-serve tier is n1.\n\nMarketing launch by next Friday.",
      transcript: [
        {
          speaker: "You",
          text: "Let's align on Q1 priorities. I've got three on the list.",
        },
        {
          speaker: "David",
          text: "Before we start — are we getting any new headcount?",
        },
        {
          speaker: "You",
          text: "No, we're staying flat. So we need to be really focused.",
        },
        { speaker: "David", text: "Got it. What are the three priorities?" },
        {
          speaker: "You",
          text: "Self-serve tier, p95 latency under 200ms, and the integrations marketplace.",
        },
      ],
    },
  },
];

function groupNotes(notes: MockNote[]) {
  const groups: Record<string, MockNote[]> = {};
  for (const n of notes) {
    if (!groups[n.group]) groups[n.group] = [];
    groups[n.group].push(n);
  }
  return groups;
}

const grouped = groupNotes(mockNotes);
const editorTabs = ["Summary", "Memos", "Transcript"];

export function AppPreviewSection() {
  return (
    <section className="mb-8 hidden px-4 md:block">
      <div className="mock-background border-color-brand relative mx-auto w-full overflow-hidden rounded-xl border px-8 py-16">
        <MockDesktopApp />
      </div>
    </section>
  );
}

function MockDesktopApp() {
  const [activeNoteId, setActiveNoteId] = useState(mockNotes[0].id);
  const [activeEditorTab, setActiveEditorTab] = useState(0);
  const [chatOpen, setChatOpen] = useState(true);
  const activeNote =
    mockNotes.find((n) => n.id === activeNoteId) || mockNotes[0];

  function handleNoteClick(noteId: string) {
    setActiveNoteId(noteId);
    setActiveEditorTab(0);
  }

  return (
    <div className="border-color-brand overflow-hidden rounded-2xl border shadow-xl">
      <div className="flex h-[720px] gap-1 overflow-hidden bg-stone-50 p-1">
        {/* Sidebar */}
        <div className="hidden shrink-0 flex-col gap-1 overflow-hidden md:flex md:w-1/5">
          <div className="flex h-9 w-full items-center py-1 pl-3">
            <div className="flex gap-2">
              <div className="size-3 rounded-full bg-red-400" />
              <div className="size-3 rounded-full bg-yellow-400" />
              <div className="size-3 rounded-full bg-green-400" />
            </div>
          </div>

          <div className="flex items-center gap-2 rounded-lg border border-neutral-200 bg-white px-3 py-1.5">
            <SearchIcon size={14} className="shrink-0 text-neutral-400" />
            <input
              type="text"
              placeholder="Search"
              className="min-w-0 flex-1 border-none bg-transparent text-sm text-neutral-900 placeholder:text-neutral-400 focus:outline-hidden"
            />
          </div>

          <div
            className={cn([
              "scrollbar-hide flex min-h-0 flex-1 flex-col overflow-y-auto",
              "rounded-xl bg-neutral-50",
            ])}
          >
            {Object.entries(grouped).map(([group, items]) => (
              <div key={group}>
                <div
                  className={cn([
                    "sticky top-0",
                    "bg-neutral-50 py-1 pr-1 pl-3",
                  ])}
                >
                  <div className="text-base font-bold text-neutral-900">
                    {group}
                  </div>
                </div>
                {items.map((note) => (
                  <button
                    key={note.id}
                    onClick={() => handleNoteClick(note.id)}
                    className={cn([
                      "w-full cursor-pointer rounded-lg px-3 py-2 text-left",
                      note.id === activeNoteId
                        ? "bg-neutral-200"
                        : "hover:bg-neutral-200/50",
                    ])}
                  >
                    <div className="flex items-center gap-2">
                      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                        <p className="pointer-events-none truncate text-sm">
                          {note.title}
                        </p>
                        <p className="font-mono text-xs text-neutral-500">
                          {note.time}
                        </p>
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            ))}
          </div>

          {/* Profile section */}
          <MockProfileSection />
        </div>

        {/* Body */}
        <div className="relative flex min-w-0 flex-1 flex-col gap-1">
          {/* Header */}
          <div className="flex h-9 w-full shrink-0 items-center">
            <div className="flex h-9 items-center py-1 pl-3 md:hidden">
              <div className="flex gap-2">
                <div className="size-3 rounded-full bg-red-400" />
                <div className="size-3 rounded-full bg-yellow-400" />
                <div className="size-3 rounded-full bg-green-400" />
              </div>
            </div>

            <div className="hidden h-full shrink-0 items-center md:flex">
              <button className="flex size-9 items-center justify-center text-neutral-400">
                <ArrowLeftIcon size={16} />
              </button>
            </div>

            <div className="relative h-full min-w-0">
              <div className="flex h-full gap-1">
                <div className="relative h-full">
                  <div
                    className={cn([
                      "relative flex items-center gap-1",
                      "h-full w-[160px] px-2",
                      "rounded-xl border",
                      "transition-colors duration-200",
                      "bg-neutral-200/50",
                      "text-black",
                      "border-stone-400",
                    ])}
                  >
                    <div className="flex min-w-0 flex-1 items-center gap-2 text-sm">
                      <div className="relative h-4 w-4 shrink-0">
                        <Icon
                          icon="mdi:note-text-outline"
                          width={16}
                          height={16}
                        />
                      </div>
                      <span className="pointer-events-none truncate">
                        {activeNote.title}
                      </span>
                    </div>
                    <X size={14} className="shrink-0 text-neutral-700" />
                  </div>
                </div>
              </div>
            </div>

            <div className="flex h-full flex-1 items-center justify-between">
              <button className="flex size-9 items-center justify-center text-neutral-600">
                <PlusIcon size={16} />
              </button>
            </div>
          </div>

          {/* Note content */}
          <div className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-xl border border-neutral-200 bg-white">
            <AnimatePresence mode="wait">
              <motion.div
                key={`${activeNote.id}-${activeEditorTab}`}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.12 }}
                className="flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <div className="w-full pt-1 pr-1 pl-2">
                  <div className="flex items-center gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="ml-1.5 flex items-center gap-1 text-xs text-neutral-600">
                        <span className="shrink-0 text-neutral-500">
                          Select folder
                        </span>
                        <ChevronRightIcon
                          size={12}
                          className="shrink-0 text-neutral-400"
                        />
                        <span className="truncate text-neutral-700">
                          {activeNote.title}
                        </span>
                      </div>
                    </div>

                    <div className="flex shrink-0 items-center">
                      <button className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs font-medium text-neutral-900">
                        <span className="size-1.5 shrink-0 rounded-full bg-red-400" />
                        <span className="whitespace-nowrap">
                          Resume listening
                        </span>
                      </button>
                      <button className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs font-medium text-neutral-600">
                        <CalendarIcon size={14} className="shrink-0" />
                        <span>2 weeks ago</span>
                      </button>
                      <button className="inline-flex size-7 items-center justify-center rounded-md text-neutral-600">
                        <EllipsisIcon size={14} />
                      </button>
                    </div>
                  </div>
                </div>

                <div className="mt-2 shrink-0 px-3">
                  <div className="flex w-full items-center gap-2">
                    <div
                      className={cn([
                        "min-w-0 flex-1",
                        "border-none bg-transparent",
                        "text-xl font-semibold",
                      ])}
                    >
                      {activeNote.noteContent.title}
                    </div>
                    <SparklesIcon
                      size={16}
                      className="shrink-0 text-neutral-400 opacity-50"
                    />
                  </div>
                </div>

                <div className="mt-2 shrink-0 px-2">
                  <div className="flex flex-col">
                    <div className="flex items-center justify-between gap-2">
                      <div className="relative min-w-0 flex-1">
                        <div className="scrollbar-hide flex items-center gap-1 overflow-x-auto">
                          {editorTabs.map((tabLabel, i) => (
                            <NoteTab
                              key={tabLabel}
                              isActive={activeEditorTab === i}
                              onClick={() => setActiveEditorTab(i)}
                            >
                              {tabLabel}
                            </NoteTab>
                          ))}
                        </div>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-4 pb-4">
                  {activeEditorTab === 0 && (
                    <div className="mock-summary text-sm leading-relaxed text-neutral-700">
                      {activeNote.noteContent.summary}
                    </div>
                  )}
                  {activeEditorTab === 1 && (
                    <textarea
                      readOnly
                      value={activeNote.noteContent.memos}
                      className="h-full w-full resize-none border-none bg-transparent text-sm leading-relaxed text-neutral-700 focus:outline-hidden"
                    />
                  )}
                  {activeEditorTab === 2 && (
                    <div className="flex flex-col gap-3">
                      {activeNote.noteContent.transcript.map((line, i) => (
                        <div key={i} className="flex gap-3">
                          <span className="w-12 shrink-0 pt-0.5 text-right text-xs font-medium text-neutral-400">
                            {line.speaker}
                          </span>
                          <span className="text-sm leading-relaxed text-neutral-700">
                            {line.text}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </motion.div>
            </AnimatePresence>

            {!chatOpen && (
              <button
                onClick={() => setChatOpen(true)}
                className={cn([
                  "absolute right-4 bottom-4 z-40",
                  "flex h-10 cursor-pointer items-center justify-center gap-1.5 rounded-full px-4",
                  "bg-white shadow-lg hover:shadow-xl",
                  "border border-neutral-200",
                  "transition-all duration-200 ease-out hover:scale-105",
                ])}
              >
                <MessageCircleIcon size={16} className="text-neutral-600" />
                <span className="text-sm font-medium text-neutral-700">
                  Chat with notes
                </span>
              </button>
            )}
          </div>
        </div>

        {/* Chat panel — full height */}
        <AnimatePresence>
          {chatOpen && (
            <motion.div
              initial={{ width: 0, opacity: 0 }}
              animate={{ width: "30%", opacity: 1 }}
              exit={{ width: 0, opacity: 0 }}
              transition={{ duration: 0.2, ease: "easeOut" }}
              className="shrink-0 overflow-hidden"
            >
              <MockChatPanel onClose={() => setChatOpen(false)} />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}

const charlieActions = [
  { icon: SlidersHorizontalIcon, label: "Actions" },
  { icon: MailIcon, label: "Draft follow-up" },
  { icon: SearchIcon, label: "Key decisions" },
];

const profileMenuItems = [
  { icon: FolderOpenIcon, label: "Folders" },
  { icon: UsersIcon, label: "Contacts" },
  { icon: CalendarIcon, label: "Calendar" },
  { icon: SearchIcon, label: "Advanced Search" },
  { divider: true },
  { icon: SparklesIcon, label: "AI Settings" },
  { icon: SettingsIcon, label: "App Settings" },
  { divider: true },
  { icon: CircleHelpIcon, label: "Help" },
] as const;

function MockProfileSection() {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="relative shrink-0">
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 10 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="border-border-subtle absolute right-0 bottom-full left-0 mb-1 overflow-hidden rounded-xl border bg-white py-1 shadow-xs"
          >
            {profileMenuItems.map((item, i) =>
              "divider" in item ? (
                <div
                  key={`d-${i}`}
                  className="border-border-subtle my-1 border-t"
                />
              ) : (
                <div key={item.label} className="px-1">
                  <div
                    className={cn([
                      "group flex w-full items-center justify-between gap-2 rounded-lg",
                      "px-3 py-1.5",
                      "text-sm whitespace-nowrap text-black",
                      "transition-colors hover:bg-neutral-100",
                    ])}
                  >
                    <div className="flex items-center justify-start gap-2.5">
                      <item.icon className="text-fg-subtle h-4 w-4 shrink-0" />
                      {item.label}
                    </div>
                  </div>
                </div>
              ),
            )}
          </motion.div>
        )}
      </AnimatePresence>

      <button
        onClick={() => setExpanded((v) => !v)}
        className={cn([
          "flex w-full cursor-pointer items-center gap-2.5 rounded-lg",
          "px-4 py-2",
          "text-left",
          "transition-all duration-300",
          expanded ? "bg-neutral-200/50" : "hover:bg-neutral-200/50",
        ])}
      >
        <div
          className={cn([
            "flex size-8 shrink-0 items-center justify-center",
            "overflow-hidden rounded-full",
            "bg-amber-100 shadow-xs",
          ])}
        >
          <span className="text-xs font-medium text-amber-800">JD</span>
        </div>
        <div className="min-w-0 flex-1 truncate text-sm text-black">
          Jane Doe
        </div>
        <ChevronUpIcon
          className={cn([
            "h-4 w-4 transition-transform duration-300",
            expanded ? "rotate-180 text-neutral-500" : "text-neutral-400",
          ])}
        />
      </button>
    </div>
  );
}

function MockChatPanel({ onClose }: { onClose: () => void }) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      {/* Top controls */}
      <div className="flex h-9 shrink-0 items-center justify-between">
        <button className="flex items-center gap-1 rounded-lg px-1.5 py-1 text-sm text-neutral-700 hover:bg-neutral-100">
          <CharLogo compact className="h-3 w-auto text-neutral-800" />
          <span className="text-xs">Ask Charlie anything</span>
          <ChevronDownIcon size={12} className="text-neutral-400" />
        </button>
        <div className="flex items-center">
          <button className="flex size-7 items-center justify-center text-neutral-400 hover:text-neutral-600">
            <PlusIcon size={14} />
          </button>
          <button className="flex size-7 items-center justify-center text-neutral-400 hover:text-neutral-600">
            <SquareArrowOutUpRightIcon size={14} />
          </button>
          <button
            onClick={onClose}
            className="flex size-7 cursor-pointer items-center justify-center text-neutral-400 hover:text-neutral-600"
          >
            <PanelRightIcon size={14} />
          </button>
        </div>
      </div>

      {/* Main area with Charlie intro */}
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="flex-1" />

        <div className="flex flex-col gap-3 px-2 pb-4">
          <div className="flex items-center gap-2">
            <CharLogo compact className="h-3.5 w-auto text-neutral-800" />
            <span className="text-sm font-semibold text-neutral-800">
              Charlie
            </span>
            <span className="rounded-full bg-blue-100 px-1.5 py-0.5 text-[10px] font-medium text-blue-600">
              Beta
            </span>
          </div>
          <p className="text-sm leading-relaxed text-neutral-500">
            Hi, I'm Charlie. I can pull context from your notes, find key
            decisions, and draft what comes next.
          </p>
          <div className="flex flex-wrap gap-1.5">
            {charlieActions.map((action) => (
              <div
                key={action.label}
                className={cn([
                  "inline-flex items-center gap-1.5 rounded-full border border-neutral-200",
                  "px-3 py-1.5 text-xs text-neutral-600",
                ])}
              >
                <action.icon size={12} className="text-neutral-400" />
                {action.label}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Context + Input */}
      <div className="shrink-0">
        <div className="flex flex-col rounded-xl border border-neutral-200 bg-white">
          <div className="flex items-center justify-between px-2.5 py-1.5">
            <div className="inline-flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-xs text-neutral-600">
              <Icon icon="mdi:note-text-outline" width={12} height={12} />
              <span className="truncate">{mockNotes[0].noteContent.title}</span>
            </div>
            <button className="flex size-5 items-center justify-center text-neutral-400">
              <PlusIcon size={12} />
            </button>
          </div>
          <div className="flex flex-col px-3 pb-2">
            <div className="mb-3 text-sm text-neutral-400">
              Ask & search about anything, or be creative!
            </div>
            <div className="flex items-center justify-end">
              <div
                className={cn([
                  "inline-flex h-7 items-center gap-1.5 rounded-lg pr-1.5 pl-2.5",
                  "text-xs font-medium",
                  "bg-neutral-800 text-white",
                ])}
              >
                <span>Send</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
