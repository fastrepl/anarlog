import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { CheckIcon, ChevronDownIcon, SearchIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { memo, useCallback, useEffect, useRef, useState } from "react";

import { Typewriter } from "@hypr/ui/components/ui/typewriter";
import { cn } from "@hypr/utils";

import { AppPreviewSection } from "@/components/app-preview";
import { CTASection as SharedCTA } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { Image } from "@/components/image";
import { HowItWorksSection } from "@/routes/_view/index";

export const Route = createFileRoute("/_view/product/ai-notetaking")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Notetaking - Char" },
      {
        name: "description",
        content:
          "Complete AI-powered notetaking solution. Record meetings, transcribe audio, and get intelligent summaries with customizable templates. Works with any video conferencing tool.",
      },
      { name: "robots", content: "noindex, nofollow" },
      { property: "og:title", content: "AI Notetaking - Char" },
      {
        property: "og:description",
        content:
          "Record meetings in real-time or upload audio files. Get instant AI transcriptions, summaries, and action items with customizable templates.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/product/ai-notetaking",
      },
    ],
  }),
});

function Component() {
  return (
    <div className="min-h-screen overflow-x-hidden">
      <div className="mx-auto">
        <HeroSection />
        <HowItWorksSection />
        <SearchSection />
        <SharingSection />
        <FloatingPanelSection />
        <SharedCTA />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-12 text-left">
          <h1 className="text-color mb-6 font-mono text-2xl tracking-wide sm:text-5xl">
            AI Notepad for Smarter Meeting Notes
          </h1>
          <p className="text-fg-muted text-lg sm:text-xl">
            You focus on the conversation. AI transcribes, summarizes,
            <br className="hidden sm:inline" /> and fills in what you missed.
          </p>
          <div className="mt-8 flex items-center gap-4">
            <DownloadButton />
            <GithubStars />
          </div>
        </header>
      </div>
      <AppPreviewSection />
    </div>
  );
}

function SearchSection() {
  const searchQueries = [
    "Q3 marketing strategy discussion",
    "client feedback on product demo",
    "budget planning for next quarter",
    "project timeline with Sarah",
    "brainstorming session notes",
  ];

  return (
    <section id="search" className="px-4 py-8">
      <div
        className="border-border bg-surface-subtle overflow-hidden rounded-xl border bg-cover bg-center"
        style={{
          backgroundImage: "url(/api/images/texture/bg-stars.jpg)",
        }}
      >
        <div className="px-6 py-20">
          <div className="flex flex-col gap-12 text-left">
            <div>
              <h2 className="mb-4 font-mono text-3xl text-stone-50">
                Find anything instantly
              </h2>
              <p className="text-base text-neutral-100">
                Search across all your notes by participant names, topics,
                keywords, or time—and jump straight to what matters
              </p>
            </div>

            <div className="relative mx-auto flex max-w-2xl flex-col gap-3">
              <div className="flex items-center gap-3 rounded-full border border-stone-300 bg-white px-4 py-3 shadow-[0_4px_6px_-1px_rgba(255,255,255,0.3),0_2px_4px_-2px_rgba(255,255,255,0.3)]">
                <SearchIcon className="size-5 shrink-0 text-stone-400" />
                <div className="min-w-0 flex-1 overflow-hidden text-left">
                  <Typewriter
                    text={searchQueries}
                    speed={100}
                    deleteSpeed={30}
                    waitTime={2000}
                    className="block truncate text-base font-light text-stone-700 sm:text-lg"
                    cursorClassName="ml-1"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const CollaboratorsCell = memo(() => {
  const [showDavid, setShowDavid] = useState(false);
  const [davidScope, setDavidScope] = useState("Can view");
  const [showPopover, setShowPopover] = useState(false);

  const baseCollaborators = [
    {
      name: "Alex Johnson",
      avatar: "/api/images/mock/alex-johnson.png",
      scope: "Can view",
    },
    {
      name: "Jessica Lee",
      avatar: "/api/images/mock/jessica-lee.png",
      scope: "Can edit",
    },
    {
      name: "Sarah Chen",
      avatar: "/api/images/mock/sarah-chen.png",
      scope: "Can edit",
    },
    {
      name: "Michael Park",
      avatar: "/api/images/mock/michael-park.png",
      scope: "Can view",
    },
    {
      name: "Emily Rodriguez",
      avatar: "/api/images/mock/emily-rodriguez.png",
      scope: "Can edit",
    },
  ];

  const davidKim = {
    name: "David Kim",
    avatar: "/api/images/mock/david-kim.png",
    scope: davidScope,
  };

  const collaborators = showDavid
    ? [...baseCollaborators, davidKim]
    : baseCollaborators;

  useEffect(() => {
    const runAnimation = () => {
      setShowDavid(false);
      setShowPopover(false);
      setDavidScope("Can view");

      const timer1 = setTimeout(() => setShowDavid(true), 2000);
      const timer2 = setTimeout(() => setShowPopover(true), 4000);
      const timer3 = setTimeout(() => {
        setDavidScope("Can comment");
        setShowPopover(false);
      }, 5000);
      const timer4 = setTimeout(() => runAnimation(), 8000);

      return () => {
        clearTimeout(timer1);
        clearTimeout(timer2);
        clearTimeout(timer3);
        clearTimeout(timer4);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <>
      <div className="h-[300px] overflow-hidden p-4 sm:aspect-4/3 sm:h-auto">
        <div className="flex h-full items-end">
          <div className="flex w-full flex-col gap-2">
            <AnimatePresence>
              {collaborators.map((person) => (
                <motion.div
                  key={person.name}
                  initial={
                    person.name === "David Kim"
                      ? { opacity: 0, x: -100 }
                      : false
                  }
                  animate={{ opacity: 1, x: 0 }}
                  transition={{
                    duration: 0.5,
                    ease: "easeOut",
                  }}
                  className="flex items-center gap-3 rounded-lg border border-stone-200/50 bg-linear-to-br from-stone-50/80 to-white/80 p-3 backdrop-blur-xs"
                >
                  <Image
                    src={person.avatar}
                    alt={person.name}
                    width={32}
                    height={32}
                    className="shrink-0 rounded-full"
                    objectFit="cover"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium text-stone-700">
                      {person.name}
                    </div>
                  </div>
                  <motion.div
                    key={`${person.name}-${person.scope}`}
                    initial={
                      person.name === "David Kim" &&
                      davidScope === "Can comment"
                        ? { scale: 1.1 }
                        : false
                    }
                    animate={{ scale: 1 }}
                    transition={{ duration: 0.3 }}
                    className="relative w-32 shrink-0"
                  >
                    <div className="flex items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700">
                      <span className="flex-1 truncate">{person.scope}</span>
                      <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
                    </div>
                    <AnimatePresence>
                      {person.name === "David Kim" && showPopover && (
                        <motion.div
                          initial={{
                            opacity: 0,
                            y: 10,
                          }}
                          animate={{
                            opacity: 1,
                            y: 0,
                          }}
                          exit={{
                            opacity: 0,
                            y: 10,
                          }}
                          transition={{
                            duration: 0.2,
                          }}
                          className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                        >
                          <div
                            className={cn([
                              "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                              davidScope === "Can view" && "bg-stone-50",
                            ])}
                          >
                            <CheckIcon
                              className={cn([
                                "h-4 w-4",
                                davidScope === "Can view"
                                  ? "text-green-600"
                                  : "text-transparent",
                              ])}
                            />
                            <span>Can view</span>
                          </div>
                          <div
                            className={cn([
                              "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                              davidScope === "Can comment" && "bg-stone-50",
                            ])}
                          >
                            <CheckIcon
                              className={cn([
                                "h-4 w-4",
                                davidScope === "Can comment"
                                  ? "text-green-600"
                                  : "text-transparent",
                              ])}
                            />
                            <span>Can comment</span>
                          </div>
                          <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                            <CheckIcon className="h-4 w-4 text-transparent" />
                            <span>Can edit</span>
                          </div>
                        </motion.div>
                      )}
                    </AnimatePresence>
                  </motion.div>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </>
  );
});

CollaboratorsCell.displayName = "CollaboratorsCell";

const ShareLinksCell = memo(() => {
  const [linkClicked, setLinkClicked] = useState(false);
  const [showCopied, setShowCopied] = useState(false);
  const [linkPermission, setLinkPermission] = useState("View only");
  const [showLinkPopover, setShowLinkPopover] = useState(false);
  const [slackClicked, setSlackClicked] = useState(false);
  const [showSlackPopover, setShowSlackPopover] = useState(false);
  const [selectedChannel, setSelectedChannel] = useState("");
  const [sendClicked, setSendClicked] = useState(false);
  const [showSent, setShowSent] = useState(false);
  const [teamsClicked, setTeamsClicked] = useState(false);
  const [showTeamsPopover, setShowTeamsPopover] = useState(false);
  const [selectedTeam, setSelectedTeam] = useState("");
  const [teamsSendClicked, setTeamsSendClicked] = useState(false);
  const [teamsShowSent, setTeamsShowSent] = useState(false);
  const [salesforceClicked, setSalesforceClicked] = useState(false);
  const [showSalesforcePopover, setShowSalesforcePopover] = useState(false);
  const [selectedLead, setSelectedLead] = useState("");
  const [salesforceSendClicked, setSalesforceSendClicked] = useState(false);
  const [salesforceShowSent, setSalesforceShowSent] = useState(false);

  useEffect(() => {
    const runAnimation = () => {
      setLinkClicked(false);
      setShowCopied(false);
      setLinkPermission("View only");
      setShowLinkPopover(false);
      setSlackClicked(false);
      setShowSlackPopover(false);
      setSelectedChannel("");
      setSendClicked(false);
      setShowSent(false);
      setTeamsClicked(false);
      setShowTeamsPopover(false);
      setSelectedTeam("");
      setTeamsSendClicked(false);
      setTeamsShowSent(false);
      setSalesforceClicked(false);
      setShowSalesforcePopover(false);
      setSelectedLead("");
      setSalesforceSendClicked(false);
      setSalesforceShowSent(false);

      const timer1 = setTimeout(() => setShowLinkPopover(true), 2000);
      const timer2 = setTimeout(() => setLinkPermission("Editable"), 2500);
      const timer3 = setTimeout(() => setShowLinkPopover(false), 2800);
      const timer4 = setTimeout(() => setLinkClicked(true), 3300);
      const timer5 = setTimeout(() => setShowCopied(true), 3600);
      const timer6 = setTimeout(() => setSlackClicked(true), 4500);
      const timer7 = setTimeout(() => setShowSlackPopover(true), 4800);
      const timer8 = setTimeout(
        () => setSelectedChannel("#team-meeting"),
        5500,
      );
      const timer9 = setTimeout(() => setShowSlackPopover(false), 5800);
      const timer10 = setTimeout(() => setSendClicked(true), 6100);
      const timer11 = setTimeout(() => setShowSent(true), 6400);
      const timer12 = setTimeout(() => setTeamsClicked(true), 7000);
      const timer13 = setTimeout(() => setShowTeamsPopover(true), 7300);
      const timer14 = setTimeout(() => setSelectedTeam("Design Team"), 8000);
      const timer15 = setTimeout(() => setShowTeamsPopover(false), 8300);
      const timer16 = setTimeout(() => setTeamsSendClicked(true), 8600);
      const timer17 = setTimeout(() => setTeamsShowSent(true), 8900);
      const timer18 = setTimeout(() => setSalesforceClicked(true), 9500);
      const timer19 = setTimeout(() => setShowSalesforcePopover(true), 9800);
      const timer20 = setTimeout(() => setSelectedLead("John Smith"), 10500);
      const timer21 = setTimeout(() => setShowSalesforcePopover(false), 10800);
      const timer22 = setTimeout(() => setSalesforceSendClicked(true), 11100);
      const timer23 = setTimeout(() => setSalesforceShowSent(true), 11400);
      const timer24 = setTimeout(() => runAnimation(), 13000);

      return () => {
        clearTimeout(timer1);
        clearTimeout(timer2);
        clearTimeout(timer3);
        clearTimeout(timer4);
        clearTimeout(timer5);
        clearTimeout(timer6);
        clearTimeout(timer7);
        clearTimeout(timer8);
        clearTimeout(timer9);
        clearTimeout(timer10);
        clearTimeout(timer11);
        clearTimeout(timer12);
        clearTimeout(timer13);
        clearTimeout(timer14);
        clearTimeout(timer15);
        clearTimeout(timer16);
        clearTimeout(timer17);
        clearTimeout(timer18);
        clearTimeout(timer19);
        clearTimeout(timer20);
        clearTimeout(timer21);
        clearTimeout(timer22);
        clearTimeout(timer23);
        clearTimeout(timer24);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <div className="flex h-[300px] items-center justify-center overflow-hidden p-4 sm:aspect-4/3 sm:h-auto">
      <div className="flex w-full flex-col gap-2">
        <motion.div
          animate={linkClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className={cn([
            "relative flex cursor-pointer items-center justify-between gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-purple-50/80 to-white/80 p-3 backdrop-blur-xs",
            showLinkPopover && "z-10",
          ])}
        >
          <Icon icon="hugeicons:note" className="w-8 text-stone-700" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={linkPermission}
              initial={
                linkPermission !== "View only" ? { scale: 1.1 } : { scale: 1 }
              }
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">{linkPermission}</span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showLinkPopover && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute top-full left-0 z-20 mt-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "Restricted" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "Restricted"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Restricted</span>
                  </div>
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "View only" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "View only"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>View only</span>
                  </div>
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      linkPermission === "Editable" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        linkPermission === "Editable"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Editable</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={showCopied ? "copied" : "copy"}
              animate={linkClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center gap-1.5 rounded-full px-3 py-2 text-xs font-medium transition-all",
                showCopied
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {showCopied && <CheckIcon className="h-4 w-4 shrink-0" />}
              <span>{showCopied ? "Copied" : "Copy"}</span>
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={slackClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className={cn([
            "relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-green-50/80 to-white/80 p-3 backdrop-blur-xs",
            showSlackPopover && "z-10",
          ])}
        >
          <Icon icon="logos:slack-icon" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedChannel || "default"}
              initial={selectedChannel ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedChannel || "Select channel"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showSlackPopover && (
                <motion.div
                  initial={{ opacity: 0, y: -10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute top-full left-0 z-20 mt-1 w-40 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedChannel === "#team-meeting" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedChannel === "#team-meeting"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>#team-meeting</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>#marketing</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>#general</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={showSent ? "sent" : "send"}
              animate={sendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                showSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {showSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Sent
                </span>
              ) : (
                "Send"
              )}
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={teamsClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className="relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-indigo-50/80 to-white/80 p-3 backdrop-blur-xs"
        >
          <Icon icon="logos:microsoft-teams" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedTeam || "default"}
              initial={selectedTeam ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedTeam || "Select team"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showTeamsPopover && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedTeam === "Design Team" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedTeam === "Design Team"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>Design Team</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Engineering</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Marketing</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={teamsShowSent ? "sent" : "send"}
              animate={teamsSendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                teamsShowSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {teamsShowSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Sent
                </span>
              ) : (
                "Send"
              )}
            </motion.button>
          </div>
        </motion.div>
        <motion.div
          animate={salesforceClicked ? { scale: [1, 0.95, 1] } : {}}
          transition={{ duration: 0.3 }}
          className="relative flex cursor-pointer items-center gap-3 overflow-visible rounded-lg border border-stone-200/50 bg-linear-to-br from-cyan-50/80 to-white/80 p-3 backdrop-blur-xs"
        >
          <Icon icon="logos:salesforce" className="w-8" />
          <div className="relative flex flex-1 items-center justify-between gap-2">
            <motion.div
              key={selectedLead || "default"}
              initial={selectedLead ? { scale: 1.1 } : { scale: 1 }}
              animate={{ scale: 1 }}
              transition={{ duration: 0.3 }}
              className="relative flex w-32 items-center gap-1 rounded border border-stone-200 bg-white px-2 py-1 text-xs text-neutral-700"
            >
              <span className="flex-1 truncate">
                {selectedLead || "Select lead"}
              </span>
              <ChevronDownIcon className="h-4 w-4 shrink-0 text-neutral-400" />
            </motion.div>
            <AnimatePresence>
              {showSalesforcePopover && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 10 }}
                  transition={{ duration: 0.2 }}
                  className="absolute bottom-full left-0 z-20 mb-1 w-32 overflow-hidden rounded border border-stone-200 bg-white shadow-lg"
                >
                  <div
                    className={cn([
                      "flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50",
                      selectedLead === "John Smith" && "bg-stone-50",
                    ])}
                  >
                    <CheckIcon
                      className={cn([
                        "h-4 w-4",
                        selectedLead === "John Smith"
                          ? "text-green-600"
                          : "text-transparent",
                      ])}
                    />
                    <span>John Smith</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Sarah Williams</span>
                  </div>
                  <div className="flex items-center gap-2 px-2 py-1.5 text-xs text-neutral-700 hover:bg-stone-50">
                    <CheckIcon className="h-4 w-4 text-transparent" />
                    <span>Mike Anderson</span>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
            <motion.button
              key={salesforceShowSent ? "synced" : "sync"}
              animate={salesforceSendClicked ? { scale: [1, 0.95, 1] } : {}}
              transition={{ duration: 0.3 }}
              className={cn([
                "flex w-24 items-center justify-center rounded-full px-3 py-2 text-xs font-medium transition-all",
                salesforceShowSent
                  ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
                  : "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 hover:scale-105 active:scale-95",
              ])}
            >
              {salesforceShowSent ? (
                <span className="flex items-center justify-center gap-1.5">
                  <CheckIcon className="h-4 w-4 shrink-0" />
                  Synced
                </span>
              ) : (
                "Sync"
              )}
            </motion.button>
          </div>
        </motion.div>
      </div>
    </div>
  );
});

ShareLinksCell.displayName = "ShareLinksCell";

const TrackProtectCell = memo(() => {
  const [countdown, setCountdown] = useState(3);
  const [showNote, setShowNote] = useState(true);
  const [showShatter, setShowShatter] = useState(false);

  useEffect(() => {
    const runAnimation = () => {
      setCountdown(3);
      setShowNote(true);
      setShowShatter(false);

      const countdownInterval = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            clearInterval(countdownInterval);
            return 0;
          }
          return prev - 1;
        });
      }, 1000);

      const shatterTimer = setTimeout(() => {
        setShowShatter(true);
        setShowNote(false);
        setTimeout(() => {
          setShowShatter(false);
          setTimeout(() => runAnimation(), 500);
        }, 800);
      }, 3000);

      return () => {
        clearInterval(countdownInterval);
        clearTimeout(shatterTimer);
      };
    };

    const cleanup = runAnimation();
    return cleanup;
  }, []);

  return (
    <div className="relative flex h-[300px] flex-col overflow-hidden bg-linear-to-br from-stone-50/30 to-stone-100/50 sm:aspect-4/3 sm:h-auto">
      <AnimatePresence>
        {countdown > 0 && showNote && (
          <motion.div
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.8 }}
            key={countdown}
            className="absolute top-2 right-2 z-10 flex h-10 w-10 items-center justify-center rounded-full border-2 border-stone-400 bg-stone-600 text-sm font-semibold text-white shadow-md"
            style={{
              background: `conic-linear(#57534e 0deg ${(4 - countdown) * 120}deg, #78716c ${(4 - countdown) * 120}deg 360deg)`,
            }}
          >
            <div className="absolute inset-1 flex items-center justify-center rounded-full bg-stone-600">
              {countdown}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="relative flex-1">
        <AnimatePresence>
          {showNote && !showShatter && (
            <motion.div
              initial={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.1 }}
              className="relative h-full overflow-hidden bg-white p-4"
            >
              <div className="pointer-events-none absolute inset-0 overflow-hidden">
                {Array.from({ length: 8 }).map((_, i) => (
                  <div
                    key={i}
                    className="absolute text-xs font-medium whitespace-nowrap text-stone-300/30"
                    style={{
                      top: `${(i * 15) % 100}%`,
                      left: `${(i * 25) % 100}%`,
                      transform: "rotate(-45deg)",
                    }}
                  >
                    user@example.com
                  </div>
                ))}
              </div>

              <div className="relative flex flex-col gap-3">
                <div className="text-sm font-semibold text-stone-700">
                  Mobile UI Update
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-5/6 rounded bg-stone-100" />
                </div>
                <div className="mt-4 text-sm font-semibold text-stone-700">
                  Dashboard Priority
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-4/5 rounded bg-stone-100" />
                </div>
                <div className="mt-4 text-sm font-semibold text-stone-700">
                  Next Steps
                </div>
                <div className="flex flex-col gap-2">
                  <div className="h-3 w-full rounded bg-stone-100" />
                  <div className="h-3 w-5/6 rounded bg-stone-100" />
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        <AnimatePresence>
          {showShatter && (
            <div className="absolute inset-0 overflow-hidden bg-white">
              {Array.from({ length: 144 }).map((_, i) => {
                const row = Math.floor(i / 12);
                const col = i % 12;
                const x = col * 8.33;
                const y = row * 8.33;
                const randomX = (Math.random() - 0.5) * 300;
                const randomY = Math.random() * 400 + 200;
                const randomRotate = (Math.random() - 0.5) * 180;

                return (
                  <motion.div
                    key={i}
                    initial={{
                      position: "absolute",
                      left: `${x}%`,
                      top: `${y}%`,
                      width: "8.33%",
                      height: "8.33%",
                      backgroundColor: "#fff",
                      border: "1px solid #e7e5e4",
                    }}
                    animate={{
                      x: randomX,
                      y: randomY,
                      rotate: randomRotate,
                      opacity: 0,
                    }}
                    transition={{
                      duration: 0.8,
                      ease: "easeIn",
                    }}
                  />
                );
              })}
            </div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
});

TrackProtectCell.displayName = "TrackProtectCell";

function SharingSection() {
  return (
    <section id="sharing" className="px-4 py-8">
      <div className="px-4 py-12 text-left lg:px-0">
        <div className="border-border mb-4 inline-block rounded-full border bg-linear-to-t from-neutral-200 to-neutral-100 px-4 py-1.5 text-xs font-medium text-neutral-900 shadow-md">
          Coming Soon
        </div>
        <h2 className="text-color mb-4 font-mono text-3xl">Share notes</h2>
        <p className="text-fg-muted text-base">
          Collaborate seamlessly by sharing meeting notes, transcripts, and
          summaries with your team.
        </p>
      </div>
      <div className="border-border overflow-hidden rounded-xl border">
        <div className="hidden min-[1000px]:grid min-[1000px]:grid-cols-3">
          <div className="border-color-brand bg-surface flex flex-col border-r">
            <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
              <div className="flex items-center gap-3">
                <Icon
                  icon="mdi:account-group"
                  className="text-color text-3xl"
                />
                <h3 className="text-color font-mono text-2xl">
                  Control who can access
                </h3>
              </div>
              <p className="text-fg-muted text-base leading-relaxed">
                Invite selected people or teams to collaborate on notes with
                granular access controls.
              </p>
            </div>
            <CollaboratorsCell />
          </div>
          <div className="border-color-brand bg-surface flex flex-col border-r">
            <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
              <div className="flex items-center gap-3">
                <Icon icon="mdi:link-variant" className="text-color text-3xl" />
                <h3 className="text-color font-mono text-2xl">
                  Share instantly
                </h3>
              </div>
              <p className="text-fg-muted text-base leading-relaxed">
                Send links or publish notes directly to Slack, Teams, or
                generate public shareable links.
              </p>
            </div>
            <ShareLinksCell />
          </div>
          <div className="bg-surface flex flex-col">
            <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
              <div className="flex items-center gap-3">
                <Icon icon="mdi:shield-lock" className="text-color text-3xl" />
                <h3 className="text-color font-mono text-2xl">
                  Track and protect
                </h3>
              </div>
              <p className="text-fg-muted text-base leading-relaxed">
                DocSend-like features including view tracking, expiration dates,
                copy protection, and watermarks.
              </p>
            </div>
            <TrackProtectCell />
          </div>
        </div>

        <div className="hidden overflow-x-auto min-[1000px]:hidden! sm:block">
          <div className="flex min-w-max">
            <div className="border-color-brand bg-surface flex w-[400px] flex-col border-r">
              <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:account-group"
                    className="text-color text-3xl"
                  />
                  <h3 className="text-color font-mono text-2xl">
                    Control who can access
                  </h3>
                </div>
                <p className="text-fg-muted text-base leading-relaxed">
                  Invite selected people or teams to collaborate on notes with
                  granular access controls.
                </p>
              </div>
              <CollaboratorsCell />
            </div>
            <div className="border-color-brand bg-surface flex w-[400px] flex-col border-r">
              <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:link-variant"
                    className="text-color text-3xl"
                  />
                  <h3 className="text-color font-mono text-2xl">
                    Share instantly
                  </h3>
                </div>
                <p className="text-fg-muted text-base leading-relaxed">
                  Send links or publish notes directly to Slack, Teams, or
                  generate public shareable links.
                </p>
              </div>
              <ShareLinksCell />
            </div>
            <div className="bg-surface flex w-[400px] flex-col">
              <div className="border-color-brand flex flex-1 flex-col gap-4 border-b p-4">
                <div className="flex items-center gap-3">
                  <Icon
                    icon="mdi:shield-lock"
                    className="text-color text-3xl"
                  />
                  <h3 className="text-color font-mono text-2xl">
                    Track and protect
                  </h3>
                </div>
                <p className="text-fg-muted text-base leading-relaxed">
                  DocSend-like features including view tracking, expiration
                  dates, copy protection, and watermarks.
                </p>
              </div>
              <TrackProtectCell />
            </div>
          </div>
        </div>

        <div className="sm:hidden">
          <div className="border-color-brand bg-surface border-b">
            <div className="border-color-brand border-b p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon
                  icon="mdi:account-group"
                  className="text-color text-2xl"
                />
                <h3 className="text-color font-mono text-xl">
                  Control who can access
                </h3>
              </div>
              <p className="text-fg-muted text-sm leading-relaxed">
                Invite selected people or teams to collaborate on notes with
                granular access controls.
              </p>
            </div>
            <CollaboratorsCell />
          </div>
          <div className="border-color-brand bg-surface border-b">
            <div className="border-color-brand border-b p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon icon="mdi:link-variant" className="text-color text-2xl" />
                <h3 className="text-color font-mono text-xl">
                  Share instantly
                </h3>
              </div>
              <p className="text-fg-muted text-sm leading-relaxed">
                Send links or publish notes directly to Slack, Teams, or
                generate public shareable links.
              </p>
            </div>
            <ShareLinksCell />
          </div>
          <div className="bg-surface">
            <div className="border-color-brand border-b p-4">
              <div className="mb-3 flex items-center gap-3">
                <Icon icon="mdi:shield-lock" className="text-color text-2xl" />
                <h3 className="text-color font-mono text-xl">
                  Track and protect
                </h3>
              </div>
              <p className="text-fg-muted text-sm leading-relaxed">
                DocSend-like features including view tracking, expiration dates,
                copy protection, and watermarks.
              </p>
            </div>
            <TrackProtectCell />
          </div>
        </div>
      </div>
    </section>
  );
}

const floatingPanelTabs = [
  {
    title: "Compact Mode",
    description:
      "Minimal overlay that indicates recording is active. Stays out of your way.",
    image: "/api/images/hyprnote/float-compact.jpg",
  },
  {
    title: "Memos",
    description:
      "Take quick notes during the meeting without losing focus on the conversation.",
    image: "/api/images/hyprnote/float-memos.jpg",
  },
  {
    title: "Transcript",
    description:
      "Watch the live transcript as the conversation unfolds in real-time.",
    image: "/api/images/hyprnote/float-transcript.jpg",
  },
  {
    title: "Live Insights",
    description:
      "Rolling summary of the past 5 minutes with AI suggestions and next steps.",
    image: "/api/images/hyprnote/float-insights.jpg",
  },
  {
    title: "Chat",
    description: "Ask questions and get instant answers during the meeting.",
    image: "/api/images/hyprnote/float-chat.jpg",
  },
];

const AUTO_ADVANCE_DURATION = 5000;

function FloatingPanelSection() {
  return (
    <section id="floating-panel" className="px-4 py-8">
      <div className="border-border overflow-hidden rounded-xl border">
        <FloatingPanelHeader />
        <FloatingPanelContent />
      </div>
    </section>
  );
}

function FloatingPanelHeader() {
  return (
    <div className="px-4 py-12 text-left md:px-8">
      <div className="border-border mb-4 inline-block rounded-full border bg-linear-to-t from-neutral-200 to-neutral-100 px-4 py-1.5 text-xs font-medium text-neutral-900 shadow-md">
        Coming Soon
      </div>
      <h2 className="text-color mb-4 font-mono text-3xl">
        Floating panel for meetings
      </h2>
      <p className="text-fg-muted max-w-3xl text-base">
        A compact overlay that stays on top during meetings but won't show when
        you share your screen.
      </p>
    </div>
  );
}

function FloatingPanelContent() {
  const [selectedTab, setSelectedTab] = useState(0);
  const [progress, setProgress] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const progressRef = useRef(0);

  const handleTabIndexChange = useCallback((nextIndex: number) => {
    setSelectedTab(nextIndex);
    setProgress(0);
    progressRef.current = 0;
  }, []);

  useEffect(() => {
    if (isPaused) return;

    const startTime =
      Date.now() - (progressRef.current / 100) * AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        const nextIndex = (selectedTab + 1) % floatingPanelTabs.length;
        setSelectedTab(nextIndex);
        setProgress(0);
        progressRef.current = 0;
        if (scrollRef.current) {
          const container = scrollRef.current;
          const scrollLeft = container.offsetWidth * nextIndex;
          container.scrollTo({
            left: scrollLeft,
            behavior: "smooth",
          });
        }
      } else {
        animationId = requestAnimationFrame(animate);
      }
    };

    animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, [selectedTab, isPaused]);

  const scrollToTab = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
    if (scrollRef.current) {
      const container = scrollRef.current;
      const scrollLeft = container.offsetWidth * index;
      container.scrollTo({ left: scrollLeft, behavior: "smooth" });
    }
  };

  const handleTabClick = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <div className="">
      <FloatingPanelMobile
        scrollRef={scrollRef}
        selectedTab={selectedTab}
        onIndexChange={handleTabIndexChange}
        scrollToTab={scrollToTab}
        progress={progress}
      />
      <FloatingPanelTablet
        selectedTab={selectedTab}
        progress={progress}
        onTabClick={handleTabClick}
        onPauseChange={setIsPaused}
      />
      <FloatingPanelDesktop />
    </div>
  );
}

function FloatingPanelTablet({
  selectedTab,
  progress,
  onTabClick,
  onPauseChange,
}: {
  selectedTab: number;
  progress: number;
  onTabClick: (index: number) => void;
  onPauseChange: (paused: boolean) => void;
}) {
  return (
    <div className="border-color-brand hidden border-t min-[800px]:max-[1000px]:block">
      <div className="flex flex-col">
        <div className="scrollbar-hide border-color-brand overflow-x-auto border-b">
          <div className="flex">
            {floatingPanelTabs.map((tab, index) => (
              <button
                key={index}
                onClick={() => onTabClick(index)}
                onMouseEnter={() =>
                  selectedTab === index && onPauseChange(true)
                }
                onMouseLeave={() =>
                  selectedTab === index && onPauseChange(false)
                }
                className={cn([
                  "border-color-brand relative flex min-w-[280px] cursor-pointer flex-col items-start overflow-hidden border-r p-6 text-left transition-colors last:border-r-0",
                  selectedTab !== index && "hover:bg-neutral-50",
                ])}
              >
                {selectedTab === index && (
                  <div
                    className="absolute inset-0 bg-stone-100 transition-none"
                    style={{ width: `${progress}%` }}
                  />
                )}
                <div className="relative">
                  <h3 className="text-color mb-1 font-mono text-base font-medium">
                    {tab.title}
                  </h3>
                  <p className="text-fg-muted text-sm">{tab.description}</p>
                </div>
              </button>
            ))}
          </div>
        </div>

        <div
          className="aspect-4/3"
          onMouseEnter={() => onPauseChange(true)}
          onMouseLeave={() => onPauseChange(false)}
        >
          <img
            src={floatingPanelTabs[selectedTab].image}
            alt={`${floatingPanelTabs[selectedTab].title} preview`}
            className="h-full w-full object-cover"
          />
        </div>
      </div>
    </div>
  );
}

function FloatingPanelDesktop() {
  const [selectedTab, setSelectedTab] = useState<number>(0);
  const [progress, setProgress] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const progressRef = useRef(0);

  useEffect(() => {
    if (isPaused) return;

    const startTime =
      Date.now() - (progressRef.current / 100) * AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        setSelectedTab((prev) => (prev + 1) % floatingPanelTabs.length);
        setProgress(0);
        progressRef.current = 0;
      } else {
        animationId = requestAnimationFrame(animate);
      }
    };

    animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, [selectedTab, isPaused]);

  const handleTabClick = (index: number) => {
    setSelectedTab(index);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <div className="border-color-brand hidden grid-cols-2 border-t min-[1000px]:grid">
      <div
        className="border-color-brand relative overflow-hidden border-r"
        style={{ paddingBottom: "56.25%" }}
      >
        <div className="absolute inset-0 overflow-y-auto">
          {floatingPanelTabs.map((tab, index) => (
            <div
              key={index}
              onClick={() => handleTabClick(index)}
              onMouseEnter={() => selectedTab === index && setIsPaused(true)}
              onMouseLeave={() => selectedTab === index && setIsPaused(false)}
              className={cn([
                "relative cursor-pointer overflow-hidden p-6 transition-colors",
                index < floatingPanelTabs.length - 1 &&
                  "border-color-brand border-b",
                selectedTab !== index && "hover:bg-neutral-50",
              ])}
            >
              {selectedTab === index && (
                <div
                  className="absolute inset-0 bg-stone-100 transition-none"
                  style={{ width: `${progress}%` }}
                />
              )}
              <div className="relative">
                <h3 className="text-color mb-1 font-mono text-base font-medium">
                  {tab.title}
                </h3>
                <p className="text-fg-muted text-sm">{tab.description}</p>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div
        className="flex aspect-4/3 items-center justify-center overflow-hidden bg-neutral-100"
        onMouseEnter={() => setIsPaused(true)}
        onMouseLeave={() => setIsPaused(false)}
      >
        <img
          src={floatingPanelTabs[selectedTab].image}
          alt={`${floatingPanelTabs[selectedTab].title} preview`}
          className="h-full w-full object-cover"
        />
      </div>
    </div>
  );
}

function FloatingPanelMobile({
  scrollRef,
  selectedTab,
  onIndexChange,
  scrollToTab,
  progress,
}: {
  scrollRef: React.RefObject<HTMLDivElement | null>;
  selectedTab: number;
  onIndexChange: (index: number) => void;
  scrollToTab: (index: number) => void;
  progress: number;
}) {
  return (
    <div className="hidden max-[800px]:block">
      <div
        ref={scrollRef}
        className="scrollbar-hide snap-x snap-mandatory overflow-x-auto"
        onScroll={(e) => {
          const container = e.currentTarget;
          const scrollLeft = container.scrollLeft;
          const itemWidth = container.offsetWidth;
          const index = Math.round(scrollLeft / itemWidth);
          if (index !== selectedTab) {
            onIndexChange(index);
          }
        }}
      >
        <div className="flex">
          {floatingPanelTabs.map((tab, index) => (
            <div key={index} className="w-full shrink-0 snap-center">
              <div className="border-color-brand flex flex-col overflow-hidden border-y">
                <div className="border-color-brand aspect-4/3 overflow-hidden border-b">
                  <img
                    src={tab.image}
                    alt={`${tab.title} preview`}
                    className="h-full w-full object-cover"
                  />
                </div>
                <div className="p-6">
                  <p className="text-fg-muted text-sm leading-relaxed">
                    <span className="text-color font-semibold">
                      {tab.title}
                    </span>{" "}
                    – {tab.description}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex justify-center gap-2 py-6">
        {floatingPanelTabs.map((_, index) => (
          <button
            key={index}
            onClick={() => scrollToTab(index)}
            className={cn([
              "h-1 cursor-pointer overflow-hidden rounded-full",
              selectedTab === index
                ? "w-8 bg-neutral-300"
                : "w-8 bg-neutral-300 hover:bg-neutral-400",
            ])}
            aria-label={`Go to tab ${index + 1}`}
          >
            {selectedTab === index && (
              <div
                className="h-full bg-stone-600 transition-none"
                style={{ width: `${progress}%` }}
              />
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
