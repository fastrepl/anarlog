import { Icon } from "@iconify-icon/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Mail, Menu, X, XIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { useIsMobile } from "@hypr/ui/hooks/use-mobile";
import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { MockWindow } from "@/components/mock-window";
import { FOUNDERS, TEAM_PHOTOS } from "@/lib/team";

type AboutSearch = {
  type?: "story" | "founder" | "photo";
  id?: string;
};

export const Route = createFileRoute("/_view/about")({
  component: Component,
  validateSearch: (search: Record<string, unknown>): AboutSearch => {
    return {
      type:
        search.type === "story" ||
        search.type === "founder" ||
        search.type === "photo"
          ? search.type
          : undefined,
      id: typeof search.id === "string" ? search.id : undefined,
    };
  },
  head: () => ({
    meta: [
      { title: "Team - Char Press Kit" },
      {
        name: "description",
        content: "Meet the Char team and download team photos.",
      },
    ],
  }),
});

type SelectedItem =
  | { type: "story" }
  | { type: "founder"; data: (typeof FOUNDERS)[number] }
  | { type: "photo"; data: (typeof TEAM_PHOTOS)[number] };

function Component() {
  const navigate = useNavigate({ from: Route.fullPath });
  const search = Route.useSearch();
  const [selectedItem, setSelectedItem] = useState<SelectedItem | null>(null);

  useEffect(() => {
    if (search.type === "story") {
      setSelectedItem({ type: "story" });
    } else if (search.type === "founder" && search.id) {
      const founder = FOUNDERS.find((f) => f.id === search.id);
      if (founder) {
        setSelectedItem({ type: "founder", data: founder });
      } else {
        setSelectedItem(null);
      }
    } else if (search.type === "photo" && search.id) {
      const photo = TEAM_PHOTOS.find((p) => p.id === search.id);
      if (photo) {
        setSelectedItem({ type: "photo", data: photo });
      } else {
        setSelectedItem(null);
      }
    } else {
      setSelectedItem(null);
    }
  }, [search.type, search.id]);

  const handleSetSelectedItem = (item: SelectedItem | null) => {
    setSelectedItem(item);
    if (item === null) {
      navigate({ search: {}, resetScroll: false });
    } else if (item.type === "story") {
      navigate({ search: { type: "story" }, resetScroll: false });
    } else if (item.type === "founder") {
      navigate({
        search: { type: "founder", id: item.data.id },
        resetScroll: false,
      });
    } else if (item.type === "photo") {
      navigate({
        search: { type: "photo", id: item.data.id },
        resetScroll: false,
      });
    }
  };

  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection />
        <AboutContentSection
          selectedItem={selectedItem}
          setSelectedItem={handleSetSelectedItem}
        />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="px-6 py-16 lg:py-24">
      <div className="mx-auto max-w-3xl text-left">
        <h1 className="mb-6 font-mono text-4xl tracking-tight text-stone-700 sm:text-5xl">
          About
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Learn about Char, meet our team, and discover the story behind our
          privacy-first note-taking platform.
        </p>
      </div>
    </div>
  );
}

function AboutContentSection({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem | null;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  const isMobile = useIsMobile();
  const [drawerOpen, setDrawerOpen] = useState(false);

  return (
    <section className="px-6 pb-16 lg:pb-24">
      <div className="mx-auto max-w-4xl">
        <MockWindow
          title="About"
          className="w-full max-w-none rounded-lg"
          prefixIcons={
            isMobile &&
            selectedItem && (
              <button
                onClick={() => setDrawerOpen(true)}
                className="rounded p-1 transition-colors hover:bg-neutral-200"
                aria-label="Open navigation"
              >
                <Menu className="h-4 w-4 text-neutral-600" />
              </button>
            )
          }
        >
          <div className="relative h-120">
            {!selectedItem ? (
              <AboutGridView setSelectedItem={setSelectedItem} />
            ) : isMobile ? (
              <>
                <MobileSidebarDrawer
                  open={drawerOpen}
                  onClose={() => setDrawerOpen(false)}
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
                <AboutDetailContent
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
              </>
            ) : (
              <AboutDetailView
                selectedItem={selectedItem}
                setSelectedItem={setSelectedItem}
              />
            )}
          </div>

          <AboutStatusBar selectedItem={selectedItem} />
        </MockWindow>
      </div>
    </section>
  );
}

function AboutGridView({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="h-120 overflow-y-auto p-8">
      <OurStoryGrid setSelectedItem={setSelectedItem} />
      <FoundersGrid setSelectedItem={setSelectedItem} />
      <TeamPhotosGrid setSelectedItem={setSelectedItem} />
    </div>
  );
}

function OurStoryGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Our Story
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        <button
          onClick={() => setSelectedItem({ type: "story" })}
          className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-left transition-colors hover:bg-stone-50"
        >
          <div className="mb-3 flex h-16 w-16 items-center justify-center">
            <Image
              src="/api/images/icons/textedit.webp"
              alt="Our Story"
              width={64}
              height={64}
              className="h-16 w-16 transition-transform group-hover:scale-110"
            />
          </div>
          <div className="font-medium text-stone-700">Our Story.txt</div>
        </button>
      </div>
    </div>
  );
}

function FoundersGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-8 border-t border-neutral-100 pt-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Founders
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {FOUNDERS.map((founder) => (
          <button
            key={founder.id}
            onClick={() =>
              setSelectedItem({
                type: "founder",
                data: founder,
              })
            }
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-left transition-colors hover:bg-stone-50"
          >
            <div className="mb-3 h-16 w-16">
              <Image
                src={founder.avatar}
                alt={founder.name}
                width={64}
                height={64}
                className="h-16 w-16 rounded-full border-2 border-neutral-200 object-cover transition-transform group-hover:scale-110"
              />
            </div>
            <div className="font-medium text-stone-700">{founder.name}</div>
          </button>
        ))}
      </div>
    </div>
  );
}

function TeamPhotosGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="border-t border-neutral-100 pt-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Team Photos
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {TEAM_PHOTOS.map((photo) => (
          <button
            key={photo.id}
            onClick={() => setSelectedItem({ type: "photo", data: photo })}
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-left transition-colors hover:bg-stone-50"
          >
            <div className="mb-3 h-16 w-16">
              <Image
                src={photo.url}
                alt={photo.name}
                width={64}
                height={64}
                className="h-16 w-16 rounded-lg border border-neutral-200 object-cover transition-transform group-hover:scale-110"
              />
            </div>
            <div className="w-full truncate text-sm font-medium text-stone-700">
              {photo.name}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function AboutDetailView({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <ResizablePanelGroup direction="horizontal" className="h-120">
      <AboutSidebar
        selectedItem={selectedItem}
        setSelectedItem={setSelectedItem}
      />
      <ResizableHandle withHandle className="w-px bg-neutral-200" />
      <AboutDetailPanel
        selectedItem={selectedItem}
        setSelectedItem={setSelectedItem}
      />
    </ResizablePanelGroup>
  );
}

function MobileSidebarDrawer({
  open,
  onClose,
  selectedItem,
  setSelectedItem,
}: {
  open: boolean;
  onClose: () => void;
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            className="absolute inset-0 z-40 bg-black/20"
            onClick={onClose}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
          />
          <motion.div
            className="absolute top-0 bottom-0 left-0 z-50 w-72 border-r border-neutral-200 bg-white shadow-lg"
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{
              type: "spring",
              damping: 25,
              stiffness: 300,
            }}
          >
            <div className="flex items-center justify-between border-b border-neutral-200 bg-stone-50 px-4 py-3">
              <span className="text-sm font-medium text-stone-700">
                Navigation
              </span>
              <button
                onClick={onClose}
                className="rounded p-1 transition-colors hover:bg-neutral-200"
                aria-label="Close drawer"
              >
                <X className="h-4 w-4 text-neutral-600" />
              </button>
            </div>
            <div className="h-[calc(100%-49px)] overflow-y-auto p-4">
              <OurStorySidebar
                selectedItem={selectedItem}
                setSelectedItem={(item) => {
                  setSelectedItem(item);
                  onClose();
                }}
              />
              <FoundersSidebar
                selectedItem={selectedItem}
                setSelectedItem={(item) => {
                  setSelectedItem(item);
                  onClose();
                }}
              />
              <TeamPhotosSidebar
                selectedItem={selectedItem}
                setSelectedItem={(item) => {
                  setSelectedItem(item);
                  onClose();
                }}
              />
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

function AboutDetailContent({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      {selectedItem?.type === "story" && (
        <StoryDetail onClose={() => setSelectedItem(null)} />
      )}
      {selectedItem?.type === "founder" && (
        <FounderDetail
          founder={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
      {selectedItem?.type === "photo" && (
        <PhotoDetail
          photo={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
    </div>
  );
}

function AboutSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <ResizablePanel defaultSize={35} minSize={25} maxSize={45}>
      <div className="h-full overflow-y-auto p-4">
        <OurStorySidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
        <FoundersSidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
        <TeamPhotosSidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
      </div>
    </ResizablePanel>
  );
}

function OurStorySidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-6">
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Our Story
      </div>
      <button
        onClick={() => setSelectedItem({ type: "story" })}
        className={cn([
          "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
          selectedItem?.type === "story"
            ? "border-stone-600 bg-stone-100"
            : "border-neutral-200",
        ])}
      >
        <div className="flex h-12 w-12 shrink-0 items-center justify-center">
          <Image
            src="/api/images/icons/textedit.webp"
            alt="Our Story"
            width={48}
            height={48}
            className="h-12 w-12"
          />
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-stone-700">
            Our Story.txt
          </p>
        </div>
      </button>
    </div>
  );
}

function FoundersSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-6">
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Founders
      </div>
      <div className="flex flex-col gap-3">
        {FOUNDERS.map((founder) => (
          <button
            key={founder.id}
            onClick={() =>
              setSelectedItem({
                type: "founder",
                data: founder,
              })
            }
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "founder" &&
              selectedItem.data.id === founder.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div className="h-12 w-12 shrink-0 overflow-hidden rounded-full border-2 border-neutral-200">
              <Image
                src={founder.avatar}
                alt={founder.name}
                width={48}
                height={48}
                className="h-full w-full object-cover"
              />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-stone-700">
                {founder.name}
              </p>
              <p className="truncate text-xs text-neutral-500">
                {founder.role}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function TeamPhotosSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div>
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Team Photos
      </div>
      <div className="flex flex-col gap-3">
        {TEAM_PHOTOS.map((photo) => (
          <button
            key={photo.id}
            onClick={() =>
              setSelectedItem({
                type: "photo",
                data: photo,
              })
            }
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "photo" &&
              selectedItem.data.id === photo.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div className="h-12 w-12 shrink-0 overflow-hidden rounded-lg border border-neutral-200">
              <Image
                src={photo.url}
                alt={photo.name}
                width={48}
                height={48}
                className="h-full w-full object-cover"
              />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-stone-700">
                {photo.name}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function AboutDetailPanel({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <ResizablePanel defaultSize={65}>
      <div className="flex h-full flex-col">
        {selectedItem?.type === "story" && (
          <StoryDetail onClose={() => setSelectedItem(null)} />
        )}
        {selectedItem?.type === "founder" && (
          <FounderDetail
            founder={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
        {selectedItem?.type === "photo" && (
          <PhotoDetail
            photo={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
      </div>
    </ResizablePanel>
  );
}

function StoryDetail({ onClose }: { onClose: () => void }) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, []);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">Our Story.txt</h2>
        <button
          onClick={onClose}
          className="cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600"
        >
          <XIcon size={16} />
        </button>
      </div>

      <div ref={scrollRef} className="overflow-y-auto p-4">
        <div className="prose prose-stone max-w-none">
          <h2 className="mb-4 font-mono text-3xl text-stone-700">
            How We Landed on Char
          </h2>
          <p className="mb-8 text-base text-neutral-500 italic">
            Our story and what we believe
          </p>

          <p className="mb-4 text-base leading-relaxed text-neutral-600">
            Char didn't start as a note-app. We were actually building an AI
            hardware toy for kids. It was fun, but for two people, hardware was
            too slow and too heavy. When we stepped back, we realized the thing
            we cared about wasn't the toy — it was helping people capture and
            understand conversations.
          </p>

          <p className="mb-4 text-base leading-relaxed text-neutral-600">
            At the same time, I was drowning in meetings and trying every AI
            notetaker out there. They were slow, distracting, or shipped every
            word to the cloud. None of them felt like something I'd trust or
            enjoy using. That became the real beginning of Char.
          </p>

          <p className="mb-8 text-base leading-relaxed text-neutral-600">
            We built the first version quickly. And it showed. Too many
            features, too many ideas, no clear philosophy. Even after YC, we
            kept moving without asking the hard questions. The product worked,
            but it didn't feel right. So we made the hard call: stop patching,
            start over. Burn it down and rebuild from scratch with a simple,
            focused point of view.
          </p>

          <h3 className="mt-8 mb-4 font-mono text-2xl text-stone-700">
            Our manifesto
          </h3>
          <p className="mb-4 text-base leading-relaxed text-neutral-600">
            We believe in the power of notetaking, not notetakers. Meetings
            should be moments of presence. If you're not adding value, your time
            is better spent elsewhere — for you and for your team.
          </p>

          <p className="mb-4 text-base leading-relaxed text-neutral-600">
            Char exists to preserve what makes us human: conversations that
            spark ideas and collaboration that moves work forward. We build
            tools that amplify human agency, not replace it. No ghost bots. No
            silent note lurkers. Just people, thinking together.
          </p>

          <p className="mb-8 text-base leading-relaxed text-neutral-600">
            We stand with those who value real connection and purposeful work.
          </p>

          <h3 className="mt-8 mb-4 font-mono text-2xl text-stone-700">
            Where we are now
          </h3>
          <p className="mb-8 text-base leading-relaxed text-neutral-600">
            Char today is the result of that reset. A fast, private, local-first
            notetaker built for people like us: meeting-heavy,
            privacy-conscious, and tired of complicated tools. It stays on your
            device. It respects your data. And it helps you think better, not
            attend meetings on autopilot.
          </p>

          <p className="mb-2 text-base leading-relaxed text-neutral-600">
            This is how we got here: a messy start, a full rewrite, and a clear
            belief that great work comes from humans — not from machines
            pretending to be in the room.
          </p>

          <div className="flex flex-col gap-2">
            <div>
              <p className="font-mono text-base font-medium text-neutral-600 italic">
                Char
              </p>
              <p className="text-sm text-neutral-500">John Jeong, Yujong Lee</p>
            </div>

            <div>
              <Image
                src="/char-signature.svg"
                alt="Char Signature"
                width={124}
                height={60}
                layout="constrained"
                className="object-contain opacity-80"
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

function FounderDetail({
  founder,
  onClose,
}: {
  founder: (typeof FOUNDERS)[number];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [founder.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{founder.name}</h2>
        <div className="flex items-center gap-2">
          <a
            href={founder.avatar}
            download={`${founder.name.toLowerCase().replace(" ", "-")}.png`}
            target="_blank"
            rel="noopener noreferrer"
            className="flex h-8 items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-4 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
          >
            Download Photo
          </a>
          <button
            onClick={onClose}
            className="cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600"
          >
            <XIcon size={16} />
          </button>
        </div>
      </div>

      <div ref={scrollRef} className="overflow-y-auto p-4">
        <div className="mb-6 flex justify-center">
          <Image
            src={founder.avatar}
            alt={founder.name}
            width={200}
            height={200}
            className="h-48 w-48 rounded-full border-4 border-neutral-200 object-cover"
          />
        </div>

        <div>
          <h3 className="mb-1 font-mono text-2xl text-stone-700">
            {founder.name}
          </h3>
          <p className="mb-4 text-sm tracking-wider text-neutral-500 uppercase">
            {founder.role}
          </p>
          <p className="mb-6 text-sm leading-relaxed text-neutral-600">
            {founder.bio}
          </p>

          <div className="flex flex-wrap gap-2">
            {founder.email && (
              <a
                href={`mailto:${founder.email}`}
                className="flex items-center gap-2 rounded-full border border-neutral-300 px-3 py-2 text-xs text-stone-700 transition-colors hover:bg-stone-50"
                aria-label="Email"
              >
                <Mail className="h-3 w-3" />
                <span>Email</span>
              </a>
            )}
            {founder.links.twitter && (
              <a
                href={founder.links.twitter}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 rounded-full border border-neutral-300 px-3 py-2 text-xs text-stone-700 transition-colors hover:bg-stone-50"
                aria-label="Twitter"
              >
                <Icon icon="mdi:twitter" className="text-sm" />
                <span>Twitter</span>
              </a>
            )}
            {founder.links.github && (
              <a
                href={founder.links.github}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 rounded-full border border-neutral-300 px-3 py-2 text-xs text-stone-700 transition-colors hover:bg-stone-50"
                aria-label="GitHub"
              >
                <Icon icon="mdi:github" className="text-sm" />
                <span>GitHub</span>
              </a>
            )}
            {founder.links.linkedin && (
              <a
                href={founder.links.linkedin}
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center gap-2 rounded-full border border-neutral-300 px-3 py-2 text-xs text-stone-700 transition-colors hover:bg-stone-50"
                aria-label="LinkedIn"
              >
                <Icon icon="mdi:linkedin" className="text-sm" />
                <span>LinkedIn</span>
              </a>
            )}
          </div>
        </div>
      </div>
    </>
  );
}

function PhotoDetail({
  photo,
  onClose,
}: {
  photo: (typeof TEAM_PHOTOS)[number];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [photo.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{photo.name}</h2>
        <div className="flex items-center gap-2">
          <a
            href={photo.url}
            download={photo.name}
            target="_blank"
            rel="noopener noreferrer"
            className="flex h-8 items-center rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 px-4 text-sm text-neutral-900 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
          >
            Download
          </a>
          <button
            onClick={onClose}
            className="cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600"
          >
            <XIcon size={16} />
          </button>
        </div>
      </div>

      <div ref={scrollRef} className="overflow-y-auto p-4">
        <Image
          src={photo.url}
          alt={photo.name}
          className="mb-6 w-full rounded-lg object-cover"
        />

        <p className="text-sm text-neutral-600">
          Team photo from the Char team.
        </p>
      </div>
    </>
  );
}

function AboutStatusBar({
  selectedItem,
}: {
  selectedItem: SelectedItem | null;
}) {
  const totalItems = 1 + FOUNDERS.length + TEAM_PHOTOS.length;

  return (
    <div className="border-t border-neutral-200 bg-stone-50 px-4 py-2">
      <span className="text-xs text-neutral-500">
        {selectedItem
          ? selectedItem.type === "founder"
            ? `Viewing ${selectedItem.data.name}`
            : selectedItem.type === "photo"
              ? `Viewing ${selectedItem.data.name}`
              : "Viewing Our Story"
          : `${totalItems} items, 3 groups`}
      </span>
    </div>
  );
}
