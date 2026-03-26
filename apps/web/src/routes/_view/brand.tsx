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

import { MockWindow } from "@/components/mock-window";

type BrandSearch = {
  type?: "visual" | "typography" | "color";
  id?: string;
};

export const Route = createFileRoute("/_view/brand")({
  component: Component,
  validateSearch: (search: Record<string, unknown>): BrandSearch => {
    return {
      type:
        search.type === "visual" ||
        search.type === "typography" ||
        search.type === "color"
          ? search.type
          : undefined,
      id: typeof search.id === "string" ? search.id : undefined,
    };
  },
  head: () => ({
    meta: [
      { title: "Brand - Char Press Kit" },
      {
        name: "description",
        content:
          "Download Char logos, icons, and brand assets. Learn about our visual identity, typography, and color palette.",
      },
    ],
  }),
});

const VISUAL_ASSETS = [
  {
    id: "icon",
    name: "Icon",
    url: "/api/images/hyprnote/icon.png",
    description: "Char app icon",
  },
  {
    id: "logo",
    name: "Logo",
    url: "/api/images/hyprnote/logo.png",
    description: "Char wordmark logo",
  },
  {
    id: "symbol-logo",
    name: "Symbol + Logo",
    url: "/api/images/hyprnote/symbol+logo.png",
    description: "Char icon with wordmark",
  },
  {
    id: "og-image",
    name: "OpenGraph Image",
    url: "/api/images/hyprnote/og-image.jpg",
    description: "Social media preview image",
  },
];

const TYPOGRAPHY = [
  {
    id: "primary-font",
    name: "Inter",
    fontFamily: "Inter",
    description:
      "Inter is our primary typeface for UI and body text. Clean, modern, and highly legible.",
    preview: "The quick brown fox jumps over the lazy dog",
    usage: "Body text, UI elements, navigation",
  },
  {
    id: "display-font",
    name: "Lora",
    fontFamily: "Lora",
    description:
      "Lora is our display typeface for headlines and featured text. Adds elegance and personality.",
    preview: "The quick brown fox jumps over the lazy dog",
    usage: "Headlines, titles, featured content",
  },
];

const COLORS = [
  {
    id: "stone-600",
    name: "Stone 600",
    hex: "#57534e",
    description: "Primary text color",
  },
  {
    id: "stone-500",
    name: "Stone 500",
    hex: "#78716c",
    description: "Secondary text color",
  },
  {
    id: "neutral-600",
    name: "Neutral 600",
    hex: "#525252",
    description: "Body text",
  },
  {
    id: "neutral-500",
    name: "Neutral 500",
    hex: "#737373",
    description: "Muted text",
  },
  {
    id: "neutral-100",
    name: "Neutral 100",
    hex: "#f5f5f5",
    description: "Background, borders",
  },
  {
    id: "stone-50",
    name: "Stone 50",
    hex: "#fafaf9",
    description: "Light backgrounds",
  },
];

type SelectedItem =
  | { type: "visual"; data: (typeof VISUAL_ASSETS)[0] }
  | { type: "typography"; data: (typeof TYPOGRAPHY)[0] }
  | { type: "color"; data: (typeof COLORS)[0] };

function Component() {
  const navigate = useNavigate({ from: Route.fullPath });
  const search = Route.useSearch();
  const [selectedItem, setSelectedItem] = useState<SelectedItem | null>(null);

  useEffect(() => {
    if (search.type === "visual" && search.id) {
      const asset = VISUAL_ASSETS.find((a) => a.id === search.id);
      if (asset) {
        setSelectedItem({ type: "visual", data: asset });
      } else {
        setSelectedItem(null);
      }
    } else if (search.type === "typography" && search.id) {
      const font = TYPOGRAPHY.find((f) => f.id === search.id);
      if (font) {
        setSelectedItem({ type: "typography", data: font });
      } else {
        setSelectedItem(null);
      }
    } else if (search.type === "color" && search.id) {
      const color = COLORS.find((c) => c.id === search.id);
      if (color) {
        setSelectedItem({ type: "color", data: color });
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
    } else if (item.type === "visual") {
      navigate({
        search: { type: "visual", id: item.data.id },
        resetScroll: false,
      });
    } else if (item.type === "typography") {
      navigate({
        search: { type: "typography", id: item.data.id },
        resetScroll: false,
      });
    } else if (item.type === "color") {
      navigate({
        search: { type: "color", id: item.data.id },
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
        <BrandContentSection
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
          Brand
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Download Char logos, icons, and brand assets. Learn about our visual
          identity, typography, and color palette.
        </p>
      </div>
    </div>
  );
}

function BrandContentSection({
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
          title="Brand"
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
              <BrandGridView setSelectedItem={setSelectedItem} />
            ) : isMobile ? (
              <>
                <MobileSidebarDrawer
                  open={drawerOpen}
                  onClose={() => setDrawerOpen(false)}
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
                <BrandDetailContent
                  selectedItem={selectedItem}
                  setSelectedItem={setSelectedItem}
                />
              </>
            ) : (
              <BrandDetailView
                selectedItem={selectedItem}
                setSelectedItem={setSelectedItem}
              />
            )}
          </div>

          <BrandStatusBar selectedItem={selectedItem} />
        </MockWindow>
      </div>
    </section>
  );
}

function BrandGridView({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="h-[480px] overflow-y-auto p-8">
      <VisualAssetsGrid setSelectedItem={setSelectedItem} />
      <TypographyGrid setSelectedItem={setSelectedItem} />
      <ColorsGrid setSelectedItem={setSelectedItem} />
    </div>
  );
}

function VisualAssetsGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Visual Assets
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {VISUAL_ASSETS.map((asset) => (
          <button
            key={asset.id}
            onClick={() => setSelectedItem({ type: "visual", data: asset })}
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-center transition-colors hover:bg-stone-50"
          >
            <div className="mb-3 flex h-16 w-16 items-center justify-center">
              <img
                src={asset.url}
                alt={asset.name}
                className="max-h-full max-w-full object-contain transition-transform group-hover:scale-110"
              />
            </div>
            <div className="font-medium text-stone-700">{asset.name}</div>
          </button>
        ))}
      </div>
    </div>
  );
}

function TypographyGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-8 border-t border-neutral-100 pt-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Typography
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {TYPOGRAPHY.map((font) => (
          <button
            key={font.id}
            onClick={() => setSelectedItem({ type: "typography", data: font })}
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-center transition-colors hover:bg-stone-50"
          >
            <div
              className="mb-3 flex h-16 w-16 items-center justify-center text-3xl font-medium text-stone-700 transition-transform group-hover:scale-110"
              style={{ fontFamily: font.fontFamily }}
            >
              Aa
            </div>
            <div
              className="font-medium text-stone-700"
              style={{ fontFamily: font.fontFamily }}
            >
              {font.name}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function ColorsGrid({
  setSelectedItem,
}: {
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="border-t border-neutral-100 pt-8">
      <div className="mb-4 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Colors
      </div>
      <div className="grid grid-cols-2 content-start gap-6 sm:grid-cols-4">
        {COLORS.map((color) => (
          <button
            key={color.id}
            onClick={() => setSelectedItem({ type: "color", data: color })}
            className="group flex h-fit cursor-pointer flex-col items-center rounded-lg p-4 text-center transition-colors hover:bg-stone-50"
          >
            <div className="mb-3 flex h-16 w-16 items-center justify-center">
              <div
                className="h-16 w-16 rounded-lg border border-neutral-200 shadow-xs transition-transform group-hover:scale-110"
                style={{ backgroundColor: color.hex }}
              />
            </div>
            <div className="font-medium text-stone-700">{color.name}</div>
          </button>
        ))}
      </div>
    </div>
  );
}

function BrandDetailView({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <ResizablePanelGroup direction="horizontal" className="h-[480px]">
      <BrandSidebar
        selectedItem={selectedItem}
        setSelectedItem={setSelectedItem}
      />
      <ResizableHandle withHandle className="w-px bg-neutral-200" />
      <BrandDetailPanel
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
              type: "tween",
              duration: 0.2,
              ease: "easeOut",
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
              <VisualAssetsSidebar
                selectedItem={selectedItem}
                setSelectedItem={(item) => {
                  setSelectedItem(item);
                  onClose();
                }}
              />
              <TypographySidebar
                selectedItem={selectedItem}
                setSelectedItem={(item) => {
                  setSelectedItem(item);
                  onClose();
                }}
              />
              <ColorsSidebar
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

function BrandDetailContent({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      {selectedItem.type === "visual" && (
        <VisualAssetDetail
          asset={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
      {selectedItem.type === "typography" && (
        <TypographyDetail
          font={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
      {selectedItem.type === "color" && (
        <ColorDetail
          color={selectedItem.data}
          onClose={() => setSelectedItem(null)}
        />
      )}
    </div>
  );
}

function BrandSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <ResizablePanel defaultSize={35} minSize={25} maxSize={45}>
      <div className="h-full overflow-y-auto p-4">
        <VisualAssetsSidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
        <TypographySidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
        <ColorsSidebar
          selectedItem={selectedItem}
          setSelectedItem={setSelectedItem}
        />
      </div>
    </ResizablePanel>
  );
}

function VisualAssetsSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-6">
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Visual Assets
      </div>
      <div className="flex flex-col gap-3">
        {VISUAL_ASSETS.map((asset) => (
          <button
            key={asset.id}
            onClick={() => setSelectedItem({ type: "visual", data: asset })}
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "visual" &&
              selectedItem.data.id === asset.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden">
              <img
                src={asset.url}
                alt={asset.name}
                className="max-h-full max-w-full object-contain"
              />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-stone-700">
                {asset.name}
              </p>
              <p className="truncate text-xs text-neutral-500">
                {asset.description}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function TypographySidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div className="mb-6">
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Typography
      </div>
      <div className="flex flex-col gap-3">
        {TYPOGRAPHY.map((font) => (
          <button
            key={font.id}
            onClick={() => setSelectedItem({ type: "typography", data: font })}
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "typography" &&
              selectedItem.data.id === font.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div
              className="flex h-12 w-12 shrink-0 items-center justify-center text-2xl font-medium text-stone-700"
              style={{ fontFamily: font.fontFamily }}
            >
              Aa
            </div>
            <div className="min-w-0 flex-1">
              <p
                className="truncate text-sm font-medium text-stone-700"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.name}
              </p>
              <p
                className="truncate text-xs text-neutral-500"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.fontFamily}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function ColorsSidebar({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem) => void;
}) {
  return (
    <div>
      <div className="mb-3 px-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
        Colors
      </div>
      <div className="flex flex-col gap-3">
        {COLORS.map((color) => (
          <button
            key={color.id}
            onClick={() => setSelectedItem({ type: "color", data: color })}
            className={cn([
              "flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-stone-50 p-3 text-left transition-colors hover:border-stone-400 hover:bg-stone-100",
              selectedItem?.type === "color" &&
              selectedItem.data.id === color.id
                ? "border-stone-600 bg-stone-100"
                : "border-neutral-200",
            ])}
          >
            <div className="flex h-12 w-12 shrink-0 items-center justify-center">
              <div
                className="h-10 w-10 rounded-lg border border-neutral-200 shadow-xs"
                style={{ backgroundColor: color.hex }}
              />
            </div>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-stone-700">
                {color.name}
              </p>
              <p className="truncate font-mono text-xs text-neutral-500">
                {color.hex}
              </p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function BrandDetailPanel({
  selectedItem,
  setSelectedItem,
}: {
  selectedItem: SelectedItem;
  setSelectedItem: (item: SelectedItem | null) => void;
}) {
  return (
    <ResizablePanel defaultSize={65}>
      <div className="flex h-full flex-col">
        {selectedItem.type === "visual" && (
          <VisualAssetDetail
            asset={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
        {selectedItem.type === "typography" && (
          <TypographyDetail
            font={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
        {selectedItem.type === "color" && (
          <ColorDetail
            color={selectedItem.data}
            onClose={() => setSelectedItem(null)}
          />
        )}
      </div>
    </ResizablePanel>
  );
}

function VisualAssetDetail({
  asset,
  onClose,
}: {
  asset: (typeof VISUAL_ASSETS)[0];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [asset.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{asset.name}</h2>
        <div className="flex items-center gap-2">
          <a
            href={asset.url}
            download
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
        <img
          src={asset.url}
          alt={asset.name}
          className="mb-6 h-auto w-full max-w-[400px] object-contain"
        />
        <p className="text-sm text-neutral-600">{asset.description}</p>
      </div>
    </>
  );
}

function TypographyDetail({
  font,
  onClose,
}: {
  font: (typeof TYPOGRAPHY)[0];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [font.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{font.name}</h2>
        <button
          onClick={onClose}
          className="cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600"
        >
          <XIcon size={16} />
        </button>
      </div>

      <div ref={scrollRef} className="overflow-y-auto p-4">
        <div className="flex flex-col gap-6">
          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Font Family
            </h3>
            <p
              className="text-lg text-stone-600"
              style={{ fontFamily: font.fontFamily }}
            >
              {font.fontFamily}
            </p>
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Description
            </h3>
            <p className="text-sm leading-relaxed text-neutral-600">
              {font.description}
            </p>
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Usage
            </h3>
            <p className="text-sm text-neutral-600">{font.usage}</p>
          </div>

          <div>
            <h3 className="mb-4 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Preview
            </h3>
            <div className="flex flex-col gap-4 rounded-lg border border-neutral-200 bg-stone-50 p-6">
              <div
                className="text-4xl text-stone-600"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.preview}
              </div>
              <div
                className="text-2xl text-stone-600"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.preview}
              </div>
              <div
                className="text-base text-stone-600"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.preview}
              </div>
              <div
                className="text-sm text-stone-600"
                style={{ fontFamily: font.fontFamily }}
              >
                {font.preview}
              </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

function ColorDetail({
  color,
  onClose,
}: {
  color: (typeof COLORS)[0];
  onClose: () => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = 0;
  }, [color.id]);

  return (
    <>
      <div className="flex items-center justify-between border-b border-neutral-200 px-4 py-2">
        <h2 className="font-medium text-stone-700">{color.name}</h2>
        <button
          onClick={onClose}
          className="cursor-pointer text-neutral-400 transition-colors hover:text-neutral-600"
        >
          <XIcon size={16} />
        </button>
      </div>

      <div ref={scrollRef} className="overflow-y-auto p-4">
        <div className="flex flex-col gap-6">
          <div>
            <div
              className="mb-4 h-48 w-full rounded-lg border border-neutral-200 shadow-xs"
              style={{ backgroundColor: color.hex }}
            />
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Hex Value
            </h3>
            <p className="font-mono text-lg text-stone-600">{color.hex}</p>
          </div>

          <div>
            <h3 className="mb-2 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
              Usage
            </h3>
            <p className="text-sm text-neutral-600">{color.description}</p>
          </div>
        </div>
      </div>
    </>
  );
}

function BrandStatusBar({
  selectedItem,
}: {
  selectedItem: SelectedItem | null;
}) {
  const totalItems = VISUAL_ASSETS.length + TYPOGRAPHY.length + COLORS.length;

  return (
    <div className="border-t border-neutral-200 bg-stone-50 px-4 py-2">
      <span className="text-xs text-neutral-500">
        {selectedItem
          ? `Viewing ${selectedItem.data.name}`
          : `${totalItems} items, 3 groups`}
      </span>
    </div>
  );
}
