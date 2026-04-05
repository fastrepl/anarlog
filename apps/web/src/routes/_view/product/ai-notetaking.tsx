import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { CheckIcon, ChevronDownIcon, SearchIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { memo, useEffect, useState } from "react";

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
    <section id="search">
      <div className="text-left">
        <div className="flex flex-col gap-12">
          <div>
            <h2 className="mb-4 font-mono text-3xl text-black">
              Find anything instantly
            </h2>
            <p className="text-base text-black/70">
              Search across all your notes by participant names, topics,
              keywords, or time—and jump straight to what matters
            </p>
          </div>

          <div className="relative flex w-full max-w-full flex-col gap-3 sm:w-[420px]">
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
    <section id="sharing" className="mt-16">
      <div className="text-left">
        <div className="border-border mb-4 inline-block rounded-full border bg-linear-to-t from-neutral-200 to-neutral-100 px-4 py-1.5 text-xs font-medium text-neutral-900 shadow-md">
          Coming Soon
        </div>
        <h2 className="text-color mb-4 font-mono text-3xl">Share notes</h2>
        <p className="text-fg-muted text-base">
          Collaborate seamlessly by sharing meeting notes, transcripts, and
          summaries with your team.
        </p>
      </div>
      <div className="border-border mt-6 overflow-hidden rounded-xl border">
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
