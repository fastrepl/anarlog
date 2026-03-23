import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Menu, X, XIcon } from "lucide-react";
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

type AppSearch = {
  type?: "screenshot";
  id?: string;
};

export const Route = createFileRoute("/_view/press-kit/app")({
  component: Component,
  validateSearch: (search: Record<string, unknown>): AppSearch => {
    return {
      type: search.type === "screenshot" ? search.type : undefined,
      id: typeof search.id === "string" ? search.id : undefined,
    };
  },
  head: () => ({
    meta: [
      { title: "App Screenshots - Char Press Kit" },
      {
        name: "description",
        content: "Download Char app screenshots and UI assets.",
      },
    ],
  }),
});

const screenshots = [
  {
    id: "float-compact",
    name: "float-compact.jpg",
    url: "/api/images/hyprnote/float-compact.jpg",
    description: "Compact floating window mode",
  },
  {
    id: "float-memos",
    name: "float-memos.jpg",
    url: "/api/images/hyprnote/float-memos.jpg",
    description: "Floating window with memos",
  },
  {
    id: "float-transcript",
    name: "float-transcript.jpg",
    url: "/api/images/hyprnote/float-transcript.jpg",
    description: "Floating window with transcript",
  },
  {
    id: "float-insights",
    name: "float-insights.jpg",
    url: "/api/images/hyprnote/float-insights.jpg",
    description: "Floating window with AI insights",
  },
  {
    id: "float-chat",
    name: "float-chat.jpg",
    url: "/api/images/hyprnote/float-chat.jpg",
    description: "Floating window with chat",
  },
  {
    id: "ai-notetaking-hero",
    name: "ai-notetaking-hero.jpg",
    url: "/api/images/hyprnote/ai-notetaking-hero.jpg",
    description: "AI notetaking hero image",
  },
  {
    id: "search-default",
    name: "search-default.jpg",
    url: "/api/images/hyprnote/mini-apps/search-default.jpg",
    description: "Search suggestions",
  },
  {
    id: "search-semantic",
    name: "search-semantic.jpg",
    url: "/api/images/hyprnote/mini-apps/search-semantic.jpg",
    description: "Semantic search",
  },
  {
    id: "search-filter",
    name: "search-filter.jpg",
    url: "/api/images/hyprnote/mini-apps/search-filter.jpg",
    description: "Search filters",
  },
];

type SelectedItem = { type: "screenshot"; data: (typeof screenshots)[0] };

function Component() {
  const navigate = useNavigate({ from: Route.fullPath });
  const search = Route.useSearch();
  const [selectedItem, setSelectedItem] = useState<SelectedItem | null>(null);

  useEffect(() => {
    if (search.type === "screenshot" && search.id) {
      const screenshot = screenshots.find((s) => s.id === search.id);
      if (screenshot) {
        setSelectedItem({ type: "screenshot", data: screenshot });
      }
    } else {
      setSelectedItem(null);
    }
  }, [search.type, search.id]);

  const handleSetSelectedItem = (item: SelectedItem | null) => {
    setSelectedItem(item);
    if (item === null) {
      navigate({ search: {}, resetScroll: false });
    } else if (item.type === "screenshot") {
      navigate({
        search: { type: "screenshot", id: item.data.id },
        resetScroll: false,
      });
    }
  };

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <AppContentSection
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
      <div className="mx-auto max-w-3xl text-center">
        <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
          App Screenshots
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Download high-quality screenshots of Char for press and marketing
          materials.
        </p>
      </div>
    </div>
  );
}

function AppContentSection({
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
          title="App Screenshots"
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
          <div className="relative h-[480px]">
            {!selectedItem ? (
              <AppGridView setSelectedItem={setSelectedItem} />
            ) : isMobile ? (
              <>
                <MobileSidebarDrawer
                  open={drawerOpen}
                  onClose={() => setDrawerOpen(false)}
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
                <AppDetailContent
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
              </>
            ) : (
              <AppDetailView
                selectedItem={selectedItem}
                setSelectedItem={setSelectedItem}
              />
            )}
          </div>

          <AppStatusBar selectedItem={selectedItem} />
        </MockWindow>
      </div>
    </section>
  );
}

function AppGridView({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="h-[480px] overflow-y-auto p-8">
      <ScreenshotsGrid setSelectedItem={setSelectedItem} />
    </div>
  );
}

function ScreenshotsGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div>
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Screenshots
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {screenshots.map((screenshot) => (
          <button
            key={screenshot.id}
            onClick={() =>
              setSelectedItem({
                type: "screenshot",
                data: screenshot,
              })
            }
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-center transition-colors hover:bg-stone-50"
          >
            <div className="mb-3 h-16 w-16">
              <Image
                src={screenshot.url}
                alt={screenshot.name}
                width={64}
                height={64}
                className="h-16 w-16 rounded-lg border border-neutral-200 object-cover transition-transform group-hover:scale-110"
              />
            </div>
            <div className="w-full truncate text-sm font-medium text-stone-700">
              {screenshot.name}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function AppDetailView({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  const sidebarScrollRef = useRef<HTMLDivElement>(null);
  const sidebarScrollPosRef = useRef<number>(0);

  useEffect(() => {
    const el = sidebarScrollRef.current;
    if (!el) return;

    el.scrollTop = sidebarScrollPosRef.current;

    const handleScroll = () => {
      sidebarScrollPosRef.current = el.scrollTop;
    };

    el.addEventListener("scroll", handleScroll);
    return () => el.removeEventListener("scroll", handleScroll);
  }, [selectedItem]);

  return (
    <ResizablePanelGroup direction="horizontal" className="h-[480px]">
      <AppSidebar
        selectedItem={selectedItem}
        setSelectedItem={setSelectedItem}
        scrollRef={sidebarScrollRef}
      />
      <ResizableHandle withHandle className="w-px bg-neutral-200" />
      <AppDetailPanel
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
              <ScreenshotsSidebar
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

function AppDetailContent({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      {selectedItem?.type === "screenshot" && (
        <ScreenshotDetail
          screenshot={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
    </div>
  );
}

function AppSidebar({
  selectedItem,
  setSelectedItem,
  scrollRef,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}) {
  return (
    <ResizablePanel defaultSize={35} minSize={25} maxSize={45}>
      <div ref={scrollRef} className="h-full overflow-y-auto p-4">
        <ScreenshotsSidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
      </div>
    </ResizablePanel>
  );
}

function ScreenshotsSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div>
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Screenshots
      </div>
      <div className="flex flex-col gap-3">
        {screenshots.map((screenshot) => (
          <button
            key={screenshot.id}
            onClick={() =>
              setSelectedItem({
                type: "screenshot",
                data: screenshot,
              })
            }
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "screenshot" &&
              selectedItem.data.id === screenshot.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div className="h-12 w-12 shrink-0 overflow-hidden rounded-lg border border-neutral-200">
              <Image
                src={screenshot.url}
                alt={screenshot.name}
                width={48}
                height={48}
                className="h-full w-full object-cover"
              />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-stone-700">
                {screenshot.name}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function AppDetailPanel({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <ResizablePanel defaultSize={65}>
      <div className="flex h-full flex-col">
        {selectedItem?.type === "screenshot" && (
          <ScreenshotDetail
            screenshot={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
      </div>
    </ResizablePanel>
  );
}

function ScreenshotDetail({
  screenshot,
  onClose,
}: {
  screenshot: (typeof screenshots)[0];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [screenshot.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{screenshot.name}</h2>
        <div className="flex items-center gap-2">
          <a
            href={screenshot.url}
            download={screenshot.name}
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
          src={screenshot.url}
          alt={screenshot.name}
          className="mb-6 w-full rounded-lg object-cover"
        />

        <p className="text-sm text-neutral-600">{screenshot.description}</p>
      </div>
    </>
  );
}

function AppStatusBar({ selectedItem }: { selectedItem: SelectedItem | null }) {
  return (
    <div className="border-t border-neutral-200 bg-stone-50 px-4 py-2">
      <span className="text-xs text-neutral-500">
        {selectedItem
          ? `Viewing ${selectedItem.data.name}`
          : `${screenshots.length} items, 1 group`}
      </span>
    </div>
  );
}
