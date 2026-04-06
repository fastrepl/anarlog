import { Icon } from "@iconify-icon/react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { allShortcuts, allTemplates } from "content-collections";
import { useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";

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
    <div className="min-h-screen">
      <div className="mx-auto">
        <HeroSection
          searchQuery={searchQuery}
          setSearchQuery={setSearchQuery}
          selectedType={selectedType}
          setSelectedType={setSelectedType}
        />
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
        "group flex cursor-pointer items-center justify-center gap-2 text-left",
        "border-color-bright border-b",
        "px-4 py-3",
        "text-fg font-mono text-sm",
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
    <div className="px-4 pt-12">
      <div className="border-brand-bright flex min-h-[60vh] flex-col justify-between rounded-xl border">
        <ContributeBanner />
        <header className="px-8 py-4 text-left lg:py-8">
          <h1 className="text-fg mb-6 font-mono text-2xl tracking-tight sm:text-6xl">
            Gallery
          </h1>
          <p className="text-fg max-w-2xl text-lg sm:text-xl">
            Browse and discover templates and shortcuts for your workflow
          </p>

          <div className="mt-8 flex flex-wrap items-center gap-4">
            <div className="flex items-center gap-2 rounded-full bg-stone-100 p-1">
              <button
                onClick={() => setSelectedType(null)}
                className={cn([
                  "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
                  selectedType === null
                    ? "text-fg bg-white shadow-xs"
                    : "text-fg-subtle hover:text-fg",
                ])}
              >
                All
              </button>
              <button
                onClick={() => setSelectedType("template")}
                className={cn([
                  "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
                  selectedType === "template"
                    ? "text-fg bg-white shadow-xs"
                    : "text-fg-subtle hover:text-fg",
                ])}
              >
                Templates
              </button>
              <button
                onClick={() => setSelectedType("shortcut")}
                className={cn([
                  "cursor-pointer rounded-full px-4 py-2 text-sm font-medium transition-colors",
                  selectedType === "shortcut"
                    ? "text-fg bg-white shadow-xs"
                    : "text-fg-subtle hover:text-fg",
                ])}
              >
                Shortcuts
              </button>
            </div>

            <div className="w-full max-w-xs">
              <div className="surface border-color-subtle relative flex items-center overflow-hidden rounded-full border-1 transition-all duration-200 focus-within:border-[var(--color-brand-dark)]">
                <input
                  type="text"
                  placeholder="Search..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="flex-1 px-4 py-2.5 text-left text-sm outline-hidden placeholder:text-left"
                />
              </div>
            </div>
          </div>
          <div className="bg-grid-dark border-color-brand mt-8 flex h-20 w-full items-center justify-start border p-8">
            <p className="text-fg font-mono text-base italic">
              "Curated by Char and the community"
            </p>
          </div>
        </header>
      </div>
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
    <div className="px-4 pt-8 pb-12 lg:pt-12 lg:pb-20">
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
                ? "bg-surface text-fg"
                : "text-fg hover:bg-surface-subtle",
            ])}
          >
            All
            <span className="text-fg-subtle ml-2 text-xs">({totalCount})</span>
          </button>
          {categories.map((category) => (
            <button
              key={category}
              onClick={() => setSelectedCategory(category)}
              className={cn([
                "w-full cursor-pointer rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors",
                selectedCategory === category
                  ? "bg-surface text-fg"
                  : "text-fg hover:bg-surface",
              ])}
            >
              {category}
              <span className="text-fg-subtle ml-2 text-xs">
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
        <div className="py-12 text-left">
          <Icon
            icon="mdi:file-search"
            className="mx-auto mb-4 text-6xl text-neutral-300"
          />
          <p className="text-fg-subtle">No items found matching your search.</p>
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
      className="group border-color-subtle bg-surface hover:border-border-bright flex cursor-pointer flex-col items-start justify-between rounded-md border p-4 text-left transition-all hover:shadow-xl"
    >
      <div className="mb-4 w-full">
        <p className="text-fg mb-2 text-xs">
          <span className="text-fg-subtle font-medium">
            {isTemplate ? "Template" : "Shortcut"}
          </span>
          <span className="text-fg-subtle mx-1.5" aria-hidden>
            ·
          </span>
          <span className="text-fg-subtle">{item.item.category}</span>
        </p>
        <h3 className="text-fg mb-2 font-mono text-lg transition-colors group-hover:underline group-hover:decoration-dotted group-hover:underline-offset-2">
          {item.item.title}
        </h3>
        <p className="text-fg line-clamp-2 text-sm">{item.item.description}</p>
      </div>
      {"targets" in item.item &&
        item.item.targets &&
        item.item.targets.length > 0 && (
          <div className="w-full pt-4">
            <div className="text-fg-subtle mb-2 text-xs font-medium tracking-wider uppercase">
              For
            </div>
            <div className="text-fg text-xs">
              {item.item.targets.join(", ")}
            </div>
          </div>
        )}
    </a>
  );
}

function ContributeCard() {
  return (
    <div className="flex flex-col items-center justify-center rounded-xs border border-dashed border-neutral-300 bg-stone-50/50 p-4 text-left">
      <h3 className="text-fg mb-2 text-center font-mono text-lg">Contribute</h3>
      <p className="text-fg mb-4 text-center text-sm">
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
