import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { CheckIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/product/mini-apps")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Mini Apps - Char" },
      {
        name: "description",
        content:
          "Built-in mini apps for contacts, calendar, daily notes, and noteshelf. Everything you need to stay organized alongside your meetings.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <ContactsSection />
        <SlashSeparator />
        <CalendarSection />
        <SlashSeparator />
        <DailyNotesSection />
        <SlashSeparator />
        <FoldersSection />
        <SlashSeparator />
        <AdvancedSearchSection />
        <SlashSeparator />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30 px-6 py-12 lg:py-20">
      <header className="mx-auto mb-12 max-w-4xl text-center">
        <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
          Everything in one place
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Built-in mini apps for contacts, calendar, daily notes, and your
          personal knowledge base.
        </p>
        <div className="mt-8">
          <Link
            to="/download/"
            className={cn([
              "inline-block rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "transition-transform hover:scale-105 active:scale-95",
            ])}
          >
            Download for free
          </Link>
        </div>
      </header>
    </div>
  );
}

function ContactsSection() {
  const [currentImage, setCurrentImage] = useState(0);

  const images = [
    "/api/images/hyprnote/mini-apps/contacts-human.jpg",
    "/api/images/hyprnote/mini-apps/contacts-org.jpg",
  ];

  useEffect(() => {
    const interval = setInterval(() => {
      setCurrentImage((prev) => (prev + 1) % images.length);
    }, 4000);

    return () => clearInterval(interval);
  }, [images.length]);

  return (
    <section id="contacts" className="bg-stone-50/30">
      <div>
        <div className="text-center">
          <div className="px-6 py-12">
            <h2 className="mb-4 font-serif text-3xl text-stone-700">
              Contacts
            </h2>
            <p className="mx-auto max-w-2xl text-base text-neutral-600">
              A relationship-focused CRM that builds itself from your meetings.
              Import contacts and watch them come alive with context once you
              actually meet.
            </p>
          </div>

          <div className="relative w-full overflow-hidden">
            {images.map((image, index) => (
              <img
                key={image}
                src={image}
                alt="Contacts interface"
                className={cn([
                  "h-auto w-full object-cover transition-opacity duration-1000",
                  index === currentImage
                    ? "opacity-100"
                    : "absolute inset-0 opacity-0",
                ])}
              />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function CalendarSection() {
  return (
    <section id="calendar" className="bg-stone-50/30">
      <div className="hidden sm:grid sm:grid-cols-2">
        <div className="flex items-center p-8">
          <div className="flex flex-col gap-4">
            <h2 className="font-serif text-3xl text-stone-700">Calendar</h2>
            <p className="text-base leading-relaxed text-neutral-600">
              Connect your calendar for intelligent meeting preparation and
              automatic note organization.
            </p>
            <ul className="flex flex-col gap-3">
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Automatic meeting linking
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Pre-meeting context and preparation
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Timeline view with notes
                </span>
              </li>
            </ul>
          </div>
        </div>
        <div className="flex items-center justify-center overflow-hidden bg-stone-50 px-8 py-8">
          <div className="w-full max-w-lg rounded-lg border-2 border-stone-200 bg-white p-6 shadow-lg">
            <div className="mb-4 flex items-start gap-4">
              <Icon
                icon="mdi:calendar"
                className="mt-1 shrink-0 text-2xl text-stone-700"
              />
              <div className="flex-1">
                <h4 className="mb-1 font-serif text-lg text-stone-700">
                  Weekly Team Sync
                </h4>
                <p className="text-sm text-neutral-600">
                  Today at 10:00 AM · 30 minutes
                </p>
              </div>
              <button className="rounded-full bg-stone-600 px-3 py-1 text-xs text-white">
                Start Recording
              </button>
            </div>
            <div className="flex flex-col gap-3">
              <div>
                <h5 className="mb-2 text-sm font-medium text-stone-700">
                  Last meeting context
                </h5>
                <div className="rounded border border-stone-300 bg-stone-50 p-3 text-xs">
                  <div className="mb-1 font-medium text-stone-900">
                    Jan 8, 2025 - Weekly Team Sync
                  </div>
                  <p className="text-stone-800">
                    Discussed Q1 roadmap, decided to prioritize mobile app.
                    Sarah to review designs by Jan 15.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="sm:hidden">
        <div className="border-b border-neutral-100 p-6">
          <h2 className="mb-3 font-serif text-2xl text-stone-700">Calendar</h2>
          <p className="mb-4 text-sm leading-relaxed text-neutral-600">
            Connect your calendar for intelligent meeting preparation and
            automatic note organization.
          </p>
          <ul className="flex flex-col gap-3">
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Automatic meeting linking
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Pre-meeting context and preparation
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Timeline view with notes
              </span>
            </li>
          </ul>
        </div>
        <div className="overflow-clip bg-stone-50 px-6 pb-0">
          <div className="rounded-lg border-2 border-stone-200 bg-white p-6 shadow-lg">
            <div className="mb-4 flex items-start gap-4">
              <Icon
                icon="mdi:calendar"
                className="mt-1 shrink-0 text-2xl text-stone-700"
              />
              <div className="flex-1">
                <h4 className="mb-1 font-serif text-lg text-stone-700">
                  Weekly Team Sync
                </h4>
                <p className="text-sm text-neutral-600">
                  Today at 10:00 AM · 30 minutes
                </p>
              </div>
              <button className="shrink-0 rounded-full bg-stone-600 px-3 py-1 text-xs text-white">
                Start Recording
              </button>
            </div>
            <div className="flex flex-col gap-3">
              <div>
                <h5 className="mb-2 text-sm font-medium text-stone-700">
                  Last meeting context
                </h5>
                <div className="rounded border border-stone-300 bg-stone-50 p-3 text-xs">
                  <div className="mb-1 font-medium text-stone-900">
                    Jan 8, 2025 - Weekly Team Sync
                  </div>
                  <p className="text-stone-800">
                    Discussed Q1 roadmap, decided to prioritize mobile app.
                    Sarah to review designs by Jan 15.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function DailyNotesSection() {
  return (
    <section id="daily-notes" className="bg-stone-50/30">
      <div className="hidden sm:grid sm:grid-cols-2">
        <div className="flex items-center p-8">
          <div className="flex flex-col gap-4">
            <h2 className="font-serif text-3xl text-stone-700">Daily Notes</h2>
            <p className="text-base leading-relaxed text-neutral-600">
              Consolidate all your meetings, action items, and insights in one
              place.
            </p>
            <ul className="flex flex-col gap-3">
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Automatic aggregation of meetings
                </span>
                <span className="rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                  Coming soon
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Chronological timeline view
                </span>
                <span className="rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                  Coming soon
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  AI-generated daily summary
                </span>
                <span className="rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                  Coming soon
                </span>
              </li>
            </ul>
          </div>
        </div>
        <div className="flex items-center justify-center overflow-hidden bg-stone-50 px-8 py-8">
          <div className="w-full max-w-lg rounded-lg border-2 border-stone-200 bg-white p-6 shadow-lg">
            <p className="text-center text-neutral-600 italic">Coming soon</p>
          </div>
        </div>
      </div>

      <div className="sm:hidden">
        <div className="border-b border-neutral-100 p-6">
          <h2 className="mb-3 font-serif text-2xl text-stone-700">
            Daily Notes
          </h2>
          <p className="mb-4 text-sm leading-relaxed text-neutral-600">
            Consolidate all your meetings, action items, and insights in one
            place.
          </p>
          <ul className="flex flex-col gap-3">
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Automatic aggregation of meetings
              </span>
              <span className="shrink-0 rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                Coming soon
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Chronological timeline view
              </span>
              <span className="shrink-0 rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                Coming soon
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                AI-generated daily summary
              </span>
              <span className="shrink-0 rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-700">
                Coming soon
              </span>
            </li>
          </ul>
        </div>
        <div className="overflow-clip bg-stone-50 px-6 pb-0">
          <div className="rounded-lg border-2 border-stone-200 bg-white p-6 shadow-lg">
            <p className="text-center text-neutral-600 italic">Coming soon</p>
          </div>
        </div>
      </div>
    </section>
  );
}

function FoldersSection() {
  return (
    <section id="folders" className="bg-stone-50/30">
      <div className="hidden sm:grid sm:grid-cols-2">
        <div className="flex items-center p-8">
          <div className="flex flex-col gap-4">
            <h2 className="font-serif text-3xl text-stone-700">Folders</h2>
            <p className="text-base leading-relaxed text-neutral-600">
              Organize your meetings and notes into folders for easy access and
              better structure.
            </p>
            <ul className="flex flex-col gap-3">
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Group related meetings together
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Organize by project, client, or topic
                </span>
              </li>
              <li className="flex items-start gap-3">
                <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
                <span className="text-neutral-600">
                  Quick navigation and filtering
                </span>
              </li>
            </ul>
          </div>
        </div>
        <div className="aspect-4/3 overflow-hidden bg-stone-50">
          <img
            src="/api/images/hyprnote/mini-apps/folders.jpg"
            alt="Folders interface"
            className="h-full w-full object-cover"
          />
        </div>
      </div>

      <div className="sm:hidden">
        <div className="border-b border-neutral-100 p-6">
          <h2 className="mb-3 font-serif text-2xl text-stone-700">Folders</h2>
          <p className="mb-4 text-sm leading-relaxed text-neutral-600">
            Organize your meetings and notes into folders for easy access and
            better structure.
          </p>
          <ul className="flex flex-col gap-3">
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Group related meetings together
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Organize by project, client, or topic
              </span>
            </li>
            <li className="flex items-start gap-3">
              <CheckIcon className="mt-0.5 size-5 shrink-0 text-green-600" />
              <span className="text-sm text-neutral-600">
                Quick navigation and filtering
              </span>
            </li>
          </ul>
        </div>
        <div className="aspect-4/3 overflow-hidden bg-stone-50">
          <img
            src="/api/images/hyprnote/mini-apps/folders.jpg"
            alt="Folders interface"
            className="h-full w-full object-cover"
          />
        </div>
      </div>
    </section>
  );
}

const ADVANCED_SEARCH_AUTO_ADVANCE_DURATION = 5000;

const advancedSearchImages = [
  {
    id: 1,
    url: "/api/images/hyprnote/mini-apps/search-default.jpg",
    title: "Suggestions",
    description:
      "Get instant search result suggestions based on recent activities",
  },
  {
    id: 2,
    url: "/api/images/hyprnote/mini-apps/search-semantic.jpg",
    title: "Semantic search",
    description: "Find relevant info even without exact keywords",
  },
  {
    id: 3,
    url: "/api/images/hyprnote/mini-apps/search-filter.jpg",
    title: "Filters",
    description: "Filter out result types easily",
  },
];

function AdvancedSearchSection() {
  const [selectedImage, setSelectedImage] = useState(1);
  const [progress, setProgress] = useState(0);
  const [isPaused, setIsPaused] = useState(false);
  const progressRef = useRef(0);

  useEffect(() => {
    if (isPaused) return;

    const startTime =
      Date.now() -
      (progressRef.current / 100) * ADVANCED_SEARCH_AUTO_ADVANCE_DURATION;
    let animationId: number;

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const newProgress = Math.min(
        (elapsed / ADVANCED_SEARCH_AUTO_ADVANCE_DURATION) * 100,
        100,
      );
      setProgress(newProgress);
      progressRef.current = newProgress;

      if (newProgress >= 100) {
        const currentIndex = advancedSearchImages.findIndex(
          (img) => img.id === selectedImage,
        );
        const nextIndex = (currentIndex + 1) % advancedSearchImages.length;
        setSelectedImage(advancedSearchImages[nextIndex].id);
        setProgress(0);
        progressRef.current = 0;
      } else {
        animationId = requestAnimationFrame(animate);
      }
    };

    animationId = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(animationId);
  }, [selectedImage, isPaused]);

  const handleTabClick = (imageId: number) => {
    setSelectedImage(imageId);
    setProgress(0);
    progressRef.current = 0;
  };

  return (
    <section id="advanced-search" className="bg-stone-50/30">
      <div>
        <div className="text-center">
          <div className="px-6 py-12">
            <div className="mb-4 inline-block rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 py-1.5 text-xs font-medium text-white opacity-50">
              Coming Soon
            </div>
            <h2 className="mb-4 font-serif text-3xl text-stone-700">
              Advanced Search
            </h2>
            <p className="text-base text-neutral-600">
              Complex queries with boolean operators and custom filters
            </p>
          </div>

          <div className="grid border-y border-neutral-100 md:grid-cols-3">
            {advancedSearchImages.map((image, index) => (
              <button
                key={image.id}
                className={cn([
                  "relative cursor-pointer overflow-hidden text-center transition-colors",
                  index < advancedSearchImages.length - 1 &&
                    "border-r border-neutral-100",
                ])}
                onClick={() => handleTabClick(image.id)}
                onMouseEnter={() =>
                  selectedImage === image.id && setIsPaused(true)
                }
                onMouseLeave={() =>
                  selectedImage === image.id && setIsPaused(false)
                }
              >
                {selectedImage === image.id && (
                  <div
                    className="absolute inset-0 bg-stone-100 transition-none"
                    style={{ width: `${progress}%` }}
                  />
                )}
                <div className="relative p-6">
                  <h3 className="mb-2 font-serif text-lg text-stone-700">
                    {image.title}
                  </h3>
                  <p className="text-sm text-neutral-600">
                    {image.description}
                  </p>
                </div>
              </button>
            ))}
          </div>

          <img
            src={
              advancedSearchImages.find((img) => img.id === selectedImage)?.url
            }
            alt="Advanced search interface"
            className="h-auto w-full object-cover"
            onMouseEnter={() => setIsPaused(true)}
            onMouseLeave={() => setIsPaused(false)}
          />
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16 lg:px-0">
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <img
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-serif text-2xl sm:text-3xl">
          Get the complete experience
        </h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Download Char to start using contacts, calendar, and folders today.
          Daily notes coming soon
        </p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
          <Link
            to="/download/"
            className={cn([
              "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
              "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "transition-all",
            ])}
          >
            Download for free
            <svg
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth="1.5"
              stroke="currentColor"
              className="ml-2 h-5 w-5 transition-transform group-hover:translate-x-1"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
              />
            </svg>
          </Link>
        </div>
      </div>
    </section>
  );
}
