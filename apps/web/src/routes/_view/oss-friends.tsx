import { Icon } from "@iconify-icon/react";
import { createFileRoute } from "@tanstack/react-router";
import { allOssFriends } from "content-collections";
import { useMemo, useState } from "react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/oss-friends")({
  component: Component,
  head: () => ({
    meta: [
      { title: "OSS Friends - Char" },
      {
        name: "description",
        content:
          "Discover amazing open source projects and tools built by our friends in the community. Char is proud to be part of the open source ecosystem.",
      },
      { property: "og:title", content: "OSS Friends - Char" },
      {
        property: "og:description",
        content:
          "Discover amazing open source projects and tools built by our friends in the community.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.com/oss-friends",
      },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:title", content: "OSS Friends - Char" },
      {
        name: "twitter:description",
        content:
          "Discover amazing open source projects and tools built by our friends in the community.",
      },
    ],
  }),
});

const INITIAL_DISPLAY_COUNT = 12;
const LOAD_MORE_COUNT = 12;

function Component() {
  const [search, setSearch] = useState("");
  const [displayCount, setDisplayCount] = useState(INITIAL_DISPLAY_COUNT);

  const filteredFriends = useMemo(() => {
    if (!search.trim()) return allOssFriends;
    const query = search.toLowerCase();
    return allOssFriends.filter(
      (friend) =>
        friend.name.toLowerCase().includes(query) ||
        friend.description.toLowerCase().includes(query),
    );
  }, [search]);

  const isSearching = search.trim().length > 0;
  const displayedFriends = isSearching
    ? filteredFriends
    : filteredFriends.slice(0, displayCount);
  const hasMore = !isSearching && displayCount < filteredFriends.length;

  const handleLoadMore = () => {
    setDisplayCount((prev) => prev + LOAD_MORE_COUNT);
  };

  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection search={search} onSearchChange={setSearch} />
        <SlashSeparator />
        <FriendsSection
          friends={displayedFriends}
          hasMore={hasMore}
          onLoadMore={handleLoadMore}
        />
        <SlashSeparator />
        <JoinSection />
      </div>
    </div>
  );
}

function HeroSection({
  search,
  onSearchChange,
}: {
  search: string;
  onSearchChange: (value: string) => void;
}) {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
      <div className="px-6 py-12 lg:py-20">
        <header className="mx-auto mb-8 max-w-4xl text-center">
          <h1 className="mb-6 font-serif text-4xl text-stone-700 sm:text-5xl lg:text-6xl">
            OSS Friends
          </h1>
          <p className="mx-auto mb-8 max-w-3xl text-lg leading-relaxed text-neutral-600 sm:text-xl">
            Discover amazing open source projects and tools built by our friends
            in the community. We believe in the power of open source and are
            proud to be part of this ecosystem.
          </p>
          <div className="mx-auto max-w-md">
            <div
              className={cn([
                "relative flex items-center",
                "rounded-full border border-neutral-200 bg-white",
                "transition-colors focus-within:border-stone-400",
              ])}
            >
              <Icon
                icon="mdi:magnify"
                className="absolute left-4 text-xl text-neutral-400"
              />
              <input
                type="text"
                placeholder="Search projects..."
                value={search}
                onChange={(e) => onSearchChange(e.target.value)}
                className="w-full rounded-full bg-transparent px-12 py-3 text-neutral-800 outline-hidden placeholder:text-neutral-400"
              />
            </div>
          </div>
        </header>
      </div>
    </div>
  );
}

function FriendsSection({
  friends,
  hasMore,
  onLoadMore,
}: {
  friends: typeof allOssFriends;
  hasMore: boolean;
  onLoadMore: () => void;
}) {
  return (
    <section>
      <div className="grid grid-cols-1 gap-px bg-neutral-200 sm:grid-cols-2 lg:grid-cols-3">
        {friends.map((friend) => (
          <a
            key={friend.slug}
            href={friend.href}
            target="_blank"
            rel="noopener noreferrer"
            className={cn([
              "group flex h-full flex-col overflow-hidden bg-white",
              "transition-all hover:bg-stone-50",
              "border-b border-neutral-200",
            ])}
          >
            <div className="aspect-40/21 shrink-0 overflow-hidden bg-neutral-100">
              <Image
                src={friend.image || "/api/images/hyprnote/default-cover.jpg"}
                alt={friend.name}
                className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                objectFit="cover"
                layout="fullWidth"
              />
            </div>
            <div className="flex flex-1 flex-col border-t border-neutral-200 p-4">
              <div className="mb-2 flex items-start justify-between gap-2">
                <h3 className="text-lg font-medium text-stone-700 group-hover:text-stone-800">
                  {friend.name}
                </h3>
                <Icon
                  icon="mdi:arrow-top-right"
                  className="shrink-0 text-lg text-neutral-400 transition-colors group-hover:text-stone-600"
                />
              </div>
              <p className="mb-3 line-clamp-2 flex-1 text-sm leading-relaxed text-neutral-600">
                {friend.description}
              </p>
              <a
                href={friend.github}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => e.stopPropagation()}
                className="mt-auto inline-flex items-center gap-1.5 text-xs text-neutral-500 transition-colors hover:text-neutral-700"
              >
                <Icon icon="mdi:github" className="text-sm" />
                <span>View on GitHub</span>
              </a>
            </div>
          </a>
        ))}
      </div>
      {hasMore && (
        <div className="flex justify-center bg-white py-8">
          <button
            onClick={onLoadMore}
            className={cn([
              "inline-flex cursor-pointer items-center justify-center gap-2 rounded-full px-6 py-3 text-base font-medium",
              "border border-neutral-200 text-neutral-700",
              "bg-linear-to-t from-stone-100 to-white",
              "transition-all hover:border-stone-300 hover:from-stone-200 hover:to-stone-50",
            ])}
          >
            Load more
          </button>
        </div>
      )}
    </section>
  );
}

function JoinSection() {
  return (
    <section className="bg-stone-50/30 px-6 py-12 lg:py-16">
      <div className="mx-auto max-w-2xl text-center">
        <h2 className="mb-4 font-serif text-2xl text-stone-700 sm:text-3xl">
          Want to be listed?
        </h2>
        <p className="mb-6 text-neutral-600">
          If you're building an open source project and would like to be
          featured on this page, we'd love to hear from you.
        </p>
        <a
          href="https://github.com/fastrepl/char/issues/new?title=OSS%20Friends%20Request&body=Project%20Name:%0AProject%20URL:%0ADescription:"
          target="_blank"
          rel="noopener noreferrer"
          className={cn([
            "inline-flex items-center justify-center gap-2 rounded-full px-6 py-3 text-base font-medium",
            "bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
            "transition-transform hover:scale-105 active:scale-95",
          ])}
        >
          <Icon icon="mdi:github" className="text-lg" />
          Submit your project
        </a>
      </div>
    </section>
  );
}
