import { Icon } from "@iconify-icon/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { allShortcuts, allTemplates } from "content-collections";
import { useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { DownloadButton } from "@/components/download-button";
import { SlashSeparator } from "@/components/slash-separator";

type GalleryType = "template" | "shortcut";

type GallerySearch = {
  type?: GalleryType;
  category?: string;
};

export const Route = createFileRoute("/_view/gallery/")({
  component: Component,
  validateSearch: (search: Record<string, unknown>): GallerySearch => {
    return {
      type:
        search.type === "template" || search.type === "shortcut"
          ? search.type
          : undefined,
      category:
        typeof search.category === "string" ? search.category : undefined,
    };
  },
  head: () => ({
    meta: [
      { title: "Templates & Shortcuts Gallery - Char" },
      {
        name: "description",
        content:
          "Discover our library of AI meeting templates and shortcuts. Get structured summaries, extract action items, and more with Char's AI-powered tools.",
      },
      {
        property: "og:title",
        content: "Templates & Shortcuts Gallery - Char",
      },
      {
        property: "og:description",
        content:
          "Browse our collection of AI meeting templates and shortcuts. From engineering standups to sales discovery calls, find the perfect tool for your workflow.",
      },
      { property: "og:type", content: "website" },
      { property: "og:url", content: "https://char.com/gallery" },
    ],
  }),
});

type GalleryItem =
  | { type: "template"; item: (typeof allTemplates)[0] }
  | { type: "shortcut"; item: (typeof allShortcuts)[0] };

function Component() {
  const navigate = useNavigate({ from: Route.fullPath });
  const search = Route.useSearch();
  const [searchQuery, setSearchQuery] = useState("");

  const selectedType = search.type || null;
  const selectedCategory = search.category || null;

  const setSelectedType = (type: GalleryType | null) => {
    navigate({
      search: {
        type: type || undefined,
        category: selectedCategory || undefined,
      },
      resetScroll: false,
    });
  };

  const setSelectedCategory = (category: string | null) => {
    navigate({
      search: {
        type: selectedType || undefined,
        category: category || undefined,
      },
      resetScroll: false,
    });
  };

  const allItems: GalleryItem[] = useMemo(() => {
    const templates: GalleryItem[] = allTemplates.map((t) => ({
      type: "template" as const,
      item: t,
    }));
    const shortcuts: GalleryItem[] = allShortcuts.map((s) => ({
      type: "shortcut" as const,
      item: s,
    }));
    return [...templates, ...shortcuts];
  }, []);

  const itemsByCategory = useMemo(() => {
    return allItems.reduce(
      (acc, item) => {
        const category = item.item.category;
        if (!acc[category]) {
          acc[category] = [];
        }
        acc[category].push(item);
        return acc;
      },
      {} as Record<string, GalleryItem[]>,
    );
  }, [allItems]);

  const categories = Object.keys(itemsByCategory).sort();

  const filteredItems = useMemo(() => {
    let items = allItems;

    if (selectedType) {
      items = items.filter((i) => i.type === selectedType);
    }

    if (selectedCategory) {
      items = items.filter((i) => i.item.category === selectedCategory);
    }

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      items = items.filter(
        (i) =>
          i.item.title.toLowerCase().includes(query) ||
          i.item.description.toLowerCase().includes(query) ||
          i.item.category.toLowerCase().includes(query),
      );
    }

    return items;
  }, [allItems, searchQuery, selectedType, selectedCategory]);

  const filteredCategories = useMemo(() => {
    if (!selectedType) return categories;
    const items = allItems.filter((i) => i.type === selectedType);
    const cats = new Set(items.map((i) => i.item.category));
    return Array.from(cats).sort();
  }, [allItems, selectedType, categories]);

  const filteredItemsByCategory = useMemo(() => {
    return filteredItems.reduce(
      (acc, item) => {
        const category = item.item.category;
        if (!acc[category]) {
          acc[category] = [];
        }
        acc[category].push(item);
        return acc;
      },
      {} as Record<string, GalleryItem[]>,
    );
  }, [filteredItems]);

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <ContributeBanner />
        <HeroSection
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          selectedType={selectedType}
          setSelectedType={setSelectedType}
        />
        <QuoteSection />
        <MobileCategoriesSection
          categories={filteredCategories}
          selectedCategory={selectedCategory}
          setSelectedCategory={setSelectedCategory}
        />
        <GallerySection
          categories={filteredCategories}
          selectedCategory={selectedCategory}
          setSelectedCategory={setSelectedCategory}
          itemsByCategory={filteredItemsByCategory}
          filteredItems={filteredItems}
        />
        <SlashSeparator />
        <CTASection />
      </div>
    </div>
  );
}

function ContributeBanner() {
  return (
    <a
      href="https://github.com/fastrepl/char/issues/new?title=Suggest%20New%20Template%2FShortcut&body=Type:%20template%0ATitle:%20Sprint%20Planning%0ACategory:%20Engineering%0ADescription:%20A%20template%20for%20capturing%20sprint%20planning%20discussions%0A%0AStructure%20(list%20of%20sections%2C%20each%20with%20a%20title%20and%20what%20to%20include):%0A-%20Sprint%20Goals:%20Key%20objectives%20for%20the%20sprint%0A-%20User%20Stories:%20Stories%20discussed%20and%20committed%0A-%20Action%20Items:%20Tasks%20assigned%20to%20team%20members"
      target="_blank"
      rel="noopener noreferrer"
      className={cn([
        "group flex cursor-pointer items-center justify-center gap-2 text-center",
        "border-b border-stone-100 bg-stone-50/70 hover:bg-stone-100/70",
        "px-4 py-3",
        "font-serif text-sm text-stone-700",
        "transition-colors",
      ])}
    >
      Have an idea? Contribute on{" "}
      <span className="inline-flex items-center gap-0.5 group-hover:underline group-hover:decoration-dotted group-hover:underline-offset-2">
        <Icon
          icon="mdi:github"
          className="inline-block align-middle text-base"
        />{" "}
        GitHub
      </span>
    </a>
  );
}

function HeroSection({
  searchQuery,
  setSearchQuery,
  selectedType,
  setSelectedType,
}: {
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  selectedType: GalleryType | null;
  setSelectedType: (type: GalleryType | null) => void;
}) {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <section className="laptop:px-0 flex flex-col items-center gap-8 px-4 py-24 text-center">
        <div className="flex max-w-3xl flex-col gap-6">
          <h1 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
            Gallery
          </h1>
          <p className="text-lg text-neutral-600 sm:text-xl">
            Browse and discover templates and shortcuts for your workflow
          </p>
        </div>

        <div className="flex items-center gap-2 rounded-full bg-stone-100 p-1">
          <button
            onClick={() => setSelectedType(null)}
            className={cn([
              "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
              selectedType === null
                ? "bg-white text-stone-800 shadow-xs"
                : "text-stone-700 hover:text-stone-800",
            ])}
          >
            All
          </button>
          <button
            onClick={() => setSelectedType("template")}
            className={cn([
              "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
              selectedType === "template"
                ? "bg-white text-stone-800 shadow-xs"
                : "text-stone-700 hover:text-stone-800",
            ])}
          >
            Templates
          </button>
          <button
            onClick={() => setSelectedType("shortcut")}
            className={cn([
              "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
              selectedType === "shortcut"
                ? "bg-white text-stone-800 shadow-xs"
                : "text-stone-700 hover:text-stone-800",
            ])}
          >
            Shortcuts
          </button>
        </div>

        <div className="w-full max-w-xs">
          <div className="relative flex items-center overflow-hidden rounded-full border-2 border-neutral-200 transition-all duration-200 focus-within:border-stone-500">
            <input
              type="text"
              placeholder="Search..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="flex-1 bg-white px-4 py-2.5 text-center text-sm outline-hidden placeholder:text-center"
            />
          </div>
        </div>
      </section>
    </div>
  );
}

function QuoteSection() {
  return (
    <div className="border-y border-neutral-100 bg-white bg-[linear-gradient(to_right,#f5f5f5_1px,transparent_1px),linear-gradient(to_bottom,#f5f5f5_1px,transparent_1px)] bg-size-[24px_24px] bg-position-[12px_12px,12px_12px] px-4 py-4 text-center">
      <p className="font-serif text-base text-stone-700 italic">
        "Curated by Char and the community"
      </p>
    </div>
  );
}

function MobileCategoriesSection({
  categories,
  selectedCategory,
  setSelectedCategory,
}: {
  categories: string[];
  selectedCategory: string | null;
  setSelectedCategory: (category: string | null) => void;
}) {
  return (
    <div className="border-b border-neutral-100 bg-stone-50 lg:hidden">
      <div className="scrollbar-hide flex overflow-x-auto">
        <button
          onClick={() => setSelectedCategory(null)}
          className={cn([
            "shrink-0 cursor-pointer border-r border-neutral-100 px-5 py-3 text-sm font-medium whitespace-nowrap transition-colors",
            selectedCategory === null
              ? "bg-stone-600 text-white"
              : "text-stone-700 hover:bg-stone-100",
          ])}
        >
          All
        </button>
        {categories.map((category) => (
          <button
            key={category}
            onClick={() => setSelectedCategory(category)}
            className={cn([
              "shrink-0 cursor-pointer border-r border-neutral-100 px-5 py-3 text-sm font-medium whitespace-nowrap transition-colors last:border-r-0",
              selectedCategory === category
                ? "bg-stone-600 text-white"
                : "text-stone-700 hover:bg-stone-100",
            ])}
          >
            {category}
          </button>
        ))}
      </div>
    </div>
  );
}

function GallerySection({
  categories,
  selectedCategory,
  setSelectedCategory,
  itemsByCategory,
  filteredItems,
}: {
  categories: string[];
  selectedCategory: string | null;
  setSelectedCategory: (category: string | null) => void;
  itemsByCategory: Record<string, GalleryItem[]>;
  filteredItems: GalleryItem[];
}) {
  return (
    <div className="px-6 pt-8 pb-12 lg:pt-12 lg:pb-20">
      <div className="flex gap-8">
        <DesktopSidebar
          categories={categories}
          selectedCategory={selectedCategory}
          setSelectedCategory={setSelectedCategory}
          itemsByCategory={itemsByCategory}
          totalCount={filteredItems.length}
        />
        <GalleryGrid filteredItems={filteredItems} />
      </div>
    </div>
  );
}

function DesktopSidebar({
  categories,
  selectedCategory,
  setSelectedCategory,
  itemsByCategory,
  totalCount,
}: {
  categories: string[];
  selectedCategory: string | null;
  setSelectedCategory: (category: string | null) => void;
  itemsByCategory: Record<string, GalleryItem[]>;
  totalCount: number;
}) {
  return (
    <aside className="hidden w-56 shrink-0 lg:block">
      <div className="sticky top-21.25">
        <h3 className="mb-4 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
          Categories
        </h3>
        <nav className="flex flex-col gap-1">
          <button
            onClick={() => setSelectedCategory(null)}
            className={cn([
              "w-full cursor-pointer rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors",
              selectedCategory === null
                ? "bg-stone-100 text-stone-800"
                : "text-stone-700 hover:bg-stone-50",
            ])}
          >
            All
            <span className="ml-2 text-xs text-neutral-400">
              ({totalCount})
            </span>
          </button>
          {categories.map((category) => (
            <button
              key={category}
              onClick={() => setSelectedCategory(category)}
              className={cn([
                "w-full cursor-pointer rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors",
                selectedCategory === category
                  ? "bg-stone-100 text-stone-800"
                  : "text-stone-700 hover:bg-stone-50",
              ])}
            >
              {category}
              <span className="ml-2 text-xs text-neutral-400">
                ({itemsByCategory[category]?.length || 0})
              </span>
            </button>
          ))}
        </nav>
      </div>
    </aside>
  );
}

function GalleryGrid({ filteredItems }: { filteredItems: GalleryItem[] }) {
  if (filteredItems.length === 0) {
    return (
      <section className="min-w-0 flex-1">
        <div className="py-12 text-center">
          <Icon
            icon="mdi:file-search"
            className="mx-auto mb-4 text-6xl text-neutral-300"
          />
          <p className="text-neutral-600">
            No items found matching your search.
          </p>
        </div>
      </section>
    );
  }

  return (
    <section className="min-w-0 flex-1">
      <div className="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
        {filteredItems.map((item) => (
          <ItemCard key={`${item.type}-${item.item.slug}`} item={item} />
        ))}
        <ContributeCard />
      </div>
    </section>
  );
}

function ItemCard({ item }: { item: GalleryItem }) {
  const isTemplate = item.type === "template";

  return (
    <a
      href={`/gallery/${item.type}/${item.item.slug}`}
      className="group flex cursor-pointer flex-col items-start rounded-xs border border-neutral-200 bg-white p-4 text-left transition-all hover:border-neutral-300 hover:shadow-md"
    >
      <div className="mb-4 w-full">
        <p className="mb-2 text-xs text-neutral-500">
          <span className="font-medium">
            {isTemplate ? "Template" : "Shortcut"}
          </span>
          <span className="mx-1">/</span>
          <span>{item.item.category}</span>
        </p>
        <h3 className="mb-1 font-serif text-lg text-stone-700 transition-colors group-hover:text-stone-800">
          {item.item.title}
        </h3>
        <p className="line-clamp-2 text-sm text-neutral-600">
          {item.item.description}
        </p>
      </div>
      {"targets" in item.item &&
        item.item.targets &&
        item.item.targets.length > 0 && (
          <div className="w-full border-t border-neutral-100 pt-4">
            <div className="mb-2 text-xs font-medium tracking-wider text-neutral-400 uppercase">
              For
            </div>
            <div className="text-xs text-stone-700">
              {item.item.targets.join(", ")}
            </div>
          </div>
        )}
    </a>
  );
}

function ContributeCard() {
  return (
    <div className="flex flex-col items-center justify-center rounded-xs border border-dashed border-neutral-300 bg-stone-50/50 p-4 text-center">
      <h3 className="mb-2 font-serif text-lg text-stone-700">Contribute</h3>
      <p className="mb-4 text-sm text-neutral-500">
        Have an idea? Submit a PR and help the community.
      </p>
      <a
        href="https://github.com/fastrepl/char/issues/new?title=Suggest%20New%20Template%2FShortcut&body=Type:%20template%0ATitle:%20Sprint%20Planning%0ACategory:%20Engineering%0ADescription:%20A%20template%20for%20capturing%20sprint%20planning%20discussions%0A%0AStructure%20(list%20of%20sections%2C%20each%20with%20a%20title%20and%20what%20to%20include):%0A-%20Sprint%20Goals:%20Key%20objectives%20for%20the%20sprint%0A-%20User%20Stories:%20Stories%20discussed%20and%20committed%0A-%20Action%20Items:%20Tasks%20assigned%20to%20team%20members"
        target="_blank"
        rel="noopener noreferrer"
        className={cn([
          "group inline-flex h-10 w-fit items-center justify-center gap-2 px-4",
          "rounded-full bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
          "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
          "cursor-pointer text-sm transition-all",
        ])}
      >
        <Icon icon="mdi:github" className="text-base" />
        Submit your idea
      </a>
    </div>
  );
}

function CTASection() {
  return (
    <section className="px-6 py-16 text-center">
      <div className="mx-auto flex max-w-2xl flex-col gap-6">
        <h2 className="font-serif text-3xl text-stone-700 sm:text-4xl">
          Ready to transform your meetings?
        </h2>
        <p className="text-lg text-neutral-600">
          Download Char and start using these templates and shortcuts to capture
          perfect meeting notes with AI.
        </p>
        <div className="flex flex-col items-center gap-4 pt-4">
          <DownloadButton />
          <p className="text-sm text-neutral-500">
            Free to use. No credit card required.
          </p>
        </div>
      </div>
    </section>
  );
}
