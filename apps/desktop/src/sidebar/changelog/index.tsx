import { CalendarIcon, ExternalLinkIcon, SparklesIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import NoteEditor from "@hypr/tiptap/editor";
import { md2json } from "@hypr/tiptap/shared";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@hypr/ui/components/ui/breadcrumb";
import { Button } from "@hypr/ui/components/ui/button";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { safeFormat } from "@hypr/utils";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { type Tab } from "~/store/zustand/tabs";

export const changelogFiles = import.meta.glob(
  "../../../../../../web/content/changelog/*.mdx",
  { query: "?raw", import: "default" },
);

export function getLatestVersion(): string | null {
  const versions = Object.keys(changelogFiles)
    .map((k) => {
      const match = k.match(/\/([^/]+)\.mdx$/);
      return match ? match[1] : null;
    })
    .filter((v): v is string => v !== null)
    .filter((v) => !v.includes("nightly"))
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));

  return versions[0] || null;
}

function parseFrontmatter(content: string): {
  date: string | null;
  body: string;
} {
  const trimmed = content.trim();
  const frontmatterMatch = trimmed.match(
    /^---\s*\n([\s\S]*?)\n---\s*\n([\s\S]*)$/,
  );

  if (!frontmatterMatch) {
    return { date: null, body: trimmed };
  }

  const frontmatterBlock = frontmatterMatch[1];
  const body = frontmatterMatch[2];

  const dateMatch = frontmatterBlock.match(/^date:\s*(.+)$/m);
  const date = dateMatch ? dateMatch[1].trim() : null;

  return { date, body };
}

function fixImageUrls(content: string): string {
  return content.replace(
    /!\[([^\]]*)\]\(\/api\/images\/([^)]+)\)/g,
    "![$1](https://auth.hyprnote.com/storage/v1/object/public/public_images/$2)",
  );
}

export const TabItemChangelog: TabItem<Extract<Tab, { type: "changelog" }>> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => (
  <TabItemBase
    icon={<SparklesIcon className="h-4 w-4" />}
    title="What's New"
    selected={tab.active}
    pinned={tab.pinned}
    tabIndex={tabIndex}
    handleCloseThis={() => handleCloseThis(tab)}
    handleSelectThis={() => handleSelectThis(tab)}
    handleCloseOthers={handleCloseOthers}
    handleCloseAll={handleCloseAll}
    handlePinThis={() => handlePinThis(tab)}
    handleUnpinThis={() => handleUnpinThis(tab)}
  />
);

export function TabContentChangelog({
  tab,
}: {
  tab: Extract<Tab, { type: "changelog" }>;
}) {
  const { current } = tab.state;

  const { content, date, loading } = useChangelogContent(current);
  const scrollRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(scrollRef);

  const changelogExtensionOptions = useMemo(
    () => ({
      onLinkOpen: (url: string) => {
        void openerCommands.openUrl(url, null);
      },
    }),
    [],
  );

  return (
    <StandardTabWrapper>
      <div className="flex h-full flex-col">
        <div className="shrink-0 pr-1 pl-2">
          <ChangelogHeader version={current} date={date} />
        </div>

        <div className="mt-2 shrink-0 px-3">
          <h1 className="text-xl font-semibold text-neutral-900">
            What's new in {current}?
          </h1>
        </div>

        <div className="relative mt-4 min-h-0 flex-1 overflow-hidden">
          {!atStart && <ScrollFadeOverlay position="top" />}
          {!atEnd && <ScrollFadeOverlay position="bottom" />}
          <div ref={scrollRef} className="h-full overflow-y-auto px-3">
            {loading ? (
              <p className="text-neutral-500">Loading...</p>
            ) : content ? (
              <NoteEditor
                initialContent={content}
                editable={false}
                extensionOptions={changelogExtensionOptions}
              />
            ) : (
              <p className="text-neutral-500">
                No changelog available for this version.
              </p>
            )}
          </div>
        </div>
      </div>
    </StandardTabWrapper>
  );
}

function ChangelogHeader({
  version,
  date,
}: {
  version: string;
  date: string | null;
}) {
  const formattedDate = date ? safeFormat(date, "MMM d, yyyy") : null;

  return (
    <div className="w-full pt-1">
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <Breadcrumb className="ml-1.5 min-w-0">
            <BreadcrumbList className="flex-nowrap gap-0.5 overflow-hidden text-xs text-neutral-700">
              <BreadcrumbItem className="shrink-0">
                <span className="text-neutral-500">Changelog</span>
              </BreadcrumbItem>
              <BreadcrumbSeparator className="shrink-0" />
              <BreadcrumbItem className="overflow-hidden">
                <BreadcrumbPage className="truncate">{version}</BreadcrumbPage>
              </BreadcrumbItem>
            </BreadcrumbList>
          </Breadcrumb>
        </div>

        <div className="flex shrink-0 items-center">
          {formattedDate && (
            <Button
              size="sm"
              variant="ghost"
              className="pointer-events-none text-neutral-600"
            >
              <CalendarIcon size={14} className="shrink-0" />
              <span>{formattedDate}</span>
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            className="gap-1.5 text-neutral-600 hover:text-black"
            onClick={() =>
              openerCommands.openUrl(
                `https://char.com/changelog/${version}`,
                null,
              )
            }
          >
            <ExternalLinkIcon size={14} />
            <span>Open in web</span>
          </Button>
        </div>
      </div>
    </div>
  );
}

async function fetchChangelogFromGitHub(
  version: string,
): Promise<string | null> {
  const url = `https://raw.githubusercontent.com/fastrepl/char/main/apps/web/content/changelog/${version}.mdx`;
  try {
    const response = await fetch(url);
    if (!response.ok) {
      return null;
    }
    return await response.text();
  } catch {
    return null;
  }
}

function processChangelogContent(raw: string): {
  content: ReturnType<typeof md2json>;
  date: string | null;
} {
  const { date, body } = parseFrontmatter(raw);
  const markdown = fixImageUrls(body);
  const json = md2json(markdown);
  return {
    content: json,
    date,
  };
}

function useChangelogContent(version: string) {
  const [content, setContent] = useState<ReturnType<typeof md2json> | null>(
    null,
  );
  const [date, setDate] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadChangelog() {
      const key = Object.keys(changelogFiles).find((k) =>
        k.endsWith(`/${version}.mdx`),
      );

      if (key) {
        try {
          const raw = (await changelogFiles[key]()) as string;
          if (cancelled) return;
          const { content: parsed, date: parsedDate } =
            processChangelogContent(raw);
          setContent(parsed);
          setDate(parsedDate);
          setLoading(false);
          return;
        } catch {}
      }

      const raw = await fetchChangelogFromGitHub(version);
      if (cancelled) return;

      if (raw) {
        const { content: parsed, date: parsedDate } =
          processChangelogContent(raw);
        setContent(parsed);
        setDate(parsedDate);
      } else {
        setContent(null);
        setDate(null);
      }
      setLoading(false);
    }

    loadChangelog();

    return () => {
      cancelled = true;
    };
  }, [version]);

  return { content, date, loading };
}
