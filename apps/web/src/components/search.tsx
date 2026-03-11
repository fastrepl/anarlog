import { useNavigate } from "@tanstack/react-router";
import { FileText, Search as SearchIcon } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@hypr/ui/components/ui/command";
import { cn } from "@hypr/utils";

function SearchKbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd
      className={cn([
        "pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center gap-1 rounded px-1 font-mono text-xs font-medium select-none",
        "border border-neutral-300",
        "bg-linear-to-b from-white to-neutral-100",
        "text-neutral-400",
        "shadow-[0_1px_0_0_rgba(0,0,0,0.1),inset_0_1px_0_0_rgba(255,255,255,0.8)]",
      ])}
    >
      {children}
    </kbd>
  );
}

function MobileSearchBar({ className }: { className?: string }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const [pagefind, setPagefind] = useState<PagefindInstance | null>(null);
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const loadPagefind = async () => {
      try {
        const pagefindPath = "/pagefind/pagefind.js";
        const pf = await import(/* @vite-ignore */ pagefindPath);
        setPagefind(pf);
      } catch {
        console.error("Failed to load pagefind");
      }
    };

    loadPagefind();
  }, []);

  const search = useCallback(
    async (searchQuery: string) => {
      if (!pagefind || !searchQuery.trim()) {
        setResults([]);
        return;
      }

      setIsLoading(true);
      try {
        const searchResults = await pagefind.search(searchQuery);
        const data = await Promise.all(
          searchResults.results.slice(0, 10).map(async (result) => {
            const resultData = await result.data();
            return {
              url: resultData.url,
              meta: resultData.meta,
              excerpt: resultData.excerpt,
            };
          }),
        );
        setResults(data);
      } catch {
        setResults([]);
      } finally {
        setIsLoading(false);
      }
    },
    [pagefind],
  );

  useEffect(() => {
    const debounceTimer = setTimeout(() => {
      search(query);
    }, 200);

    return () => clearTimeout(debounceTimer);
  }, [query, search]);

  const handleSelect = (url: string) => {
    setQuery("");
    setResults([]);
    setIsFocused(false);
    navigate({ to: url });
  };

  const showResults = isFocused && query;

  return (
    <div className={className}>
      <div
        className={cn(["flex items-center gap-3 px-3 py-2", "bg-transparent"])}
      >
        <SearchIcon size={16} className="shrink-0 text-neutral-500" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onFocus={() => setIsFocused(true)}
          onBlur={() => {
            setTimeout(() => setIsFocused(false), 200);
          }}
          placeholder="Search docs..."
          className={cn([
            "flex-1 text-sm text-neutral-900",
            "border-none bg-transparent",
            "placeholder:text-neutral-500 focus:outline-hidden",
          ])}
        />
      </div>

      {showResults && (
        <div className="border-t border-neutral-200 bg-white">
          {isLoading && (
            <div className="px-3 py-4 text-sm text-neutral-500">
              Searching...
            </div>
          )}
          {!isLoading && results.length === 0 && (
            <div className="px-3 py-4 text-sm text-neutral-500">
              No results found
            </div>
          )}
          {!isLoading && results.length > 0 && (
            <div className="max-h-80 overflow-y-auto">
              {results.map((result, index) => (
                <button
                  key={`${result.url}-${index}`}
                  onClick={() => handleSelect(result.url)}
                  className="flex w-full items-start gap-3 border-b border-neutral-100 px-3 py-3 text-left transition-colors last:border-b-0 hover:bg-neutral-50"
                >
                  <FileText
                    size={16}
                    className="mt-0.5 shrink-0 text-neutral-400"
                  />
                  <div className="flex min-w-0 flex-col gap-1">
                    <span className="text-sm font-medium text-neutral-900">
                      {result.meta?.title || result.url}
                    </span>
                    {result.excerpt && (
                      <span
                        className="line-clamp-2 text-xs text-neutral-500"
                        dangerouslySetInnerHTML={{
                          __html: result.excerpt,
                        }}
                      />
                    )}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

interface SearchResult {
  url: string;
  meta: {
    title?: string;
  };
  excerpt: string;
}

interface PagefindResult {
  url: string;
  meta: {
    title?: string;
  };
  excerpt: string;
}

interface PagefindSearchResult {
  id: string;
  data: () => Promise<PagefindResult>;
}

interface PagefindInstance {
  search: (query: string) => Promise<{ results: PagefindSearchResult[] }>;
}

const SearchPaletteContext = createContext<{
  open: boolean;
  setOpen: (open: boolean) => void;
} | null>(null);

function useSearchPalette() {
  const ctx = useContext(SearchPaletteContext);
  if (!ctx) {
    return { open: false, setOpen: () => {} };
  }
  return ctx;
}

export function SearchPaletteProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, []);

  return (
    <SearchPaletteContext.Provider value={{ open, setOpen }}>
      {children}
      <SearchCommandPalette open={open} onOpenChange={setOpen} />
    </SearchPaletteContext.Provider>
  );
}

export function SearchTrigger({
  className,
  variant = "default",
}: {
  className?: string;
  variant?: "default" | "sidebar" | "mobile" | "header";
}) {
  const { setOpen } = useSearchPalette();

  if (variant === "sidebar") {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={cn([
          "group flex w-full items-center gap-2 px-3 py-2",
          "text-sm text-neutral-500",
          "bg-neutral-50 hover:bg-neutral-100",
          "rounded-md border border-neutral-200",
          "transition-colors",
          className,
        ])}
      >
        <SearchIcon size={16} className="text-neutral-400" />
        <span className="flex-1 text-left">Search docs...</span>
        <SearchKbd>⌘ K</SearchKbd>
      </button>
    );
  }

  if (variant === "mobile") {
    return <MobileSearchBar className={className} />;
  }

  if (variant === "header") {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={cn([
          "group flex cursor-pointer items-center gap-1.5",
          "text-neutral-400 hover:text-neutral-600",
          "transition-colors",
          className,
        ])}
      >
        <SearchIcon size={16} />
        <SearchKbd>⌘ K</SearchKbd>
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={() => setOpen(true)}
      className={cn([
        "flex h-8 items-center gap-2 px-3",
        "text-sm text-neutral-500",
        "bg-neutral-50 hover:bg-neutral-100",
        "rounded-full border border-neutral-200",
        "transition-colors",
        className,
      ])}
    >
      <SearchIcon size={14} className="text-neutral-400" />
      <span className="hidden lg:inline">Search</span>
      <SearchKbd>⌘ K</SearchKbd>
    </button>
  );
}

function SearchCommandPalette({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [pagefind, setPagefind] = useState<PagefindInstance | null>(null);
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-focus input when palette opens
  useEffect(() => {
    if (open) {
      // Small delay to ensure the portal is mounted
      const timer = setTimeout(() => {
        inputRef.current?.focus();
      }, 10);
      return () => clearTimeout(timer);
    }
  }, [open]);

  useEffect(() => {
    const loadPagefind = async () => {
      try {
        const pagefindPath = "/pagefind/pagefind.js";
        const pf = await import(/* @vite-ignore */ pagefindPath);
        setPagefind(pf);
      } catch {
        console.error("Failed to load pagefind");
      }
    };

    if (open && !pagefind) {
      loadPagefind();
    }
  }, [open, pagefind]);

  const search = useCallback(
    async (searchQuery: string) => {
      if (!pagefind || !searchQuery.trim()) {
        setResults([]);
        return;
      }

      setIsLoading(true);
      try {
        const searchResults = await pagefind.search(searchQuery);
        const data = await Promise.all(
          searchResults.results.slice(0, 10).map(async (result) => {
            const resultData = await result.data();
            return {
              url: resultData.url,
              meta: resultData.meta,
              excerpt: resultData.excerpt,
            };
          }),
        );
        setResults(data);
      } catch {
        setResults([]);
      } finally {
        setIsLoading(false);
      }
    },
    [pagefind],
  );

  useEffect(() => {
    const debounceTimer = setTimeout(() => {
      search(query);
    }, 200);

    return () => clearTimeout(debounceTimer);
  }, [query, search]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onOpenChange(false);
      }
    };

    if (open) {
      document.addEventListener("keydown", handleEscape);
      return () => document.removeEventListener("keydown", handleEscape);
    }
  }, [open, onOpenChange]);

  useEffect(() => {
    const zE = (window as { zE?: (...args: unknown[]) => void }).zE;
    if (zE) {
      if (open) {
        zE("messenger", "hide");
      } else {
        zE("messenger", "show");
      }
    }
  }, [open]);

  const handleSelect = (url: string) => {
    onOpenChange(false);
    setQuery("");
    setResults([]);
    navigate({ to: url });
  };

  if (!open) return null;

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      className="fixed inset-x-0 top-17.25 bottom-0 z-9999 backdrop-blur-xs"
      onClick={() => onOpenChange(false)}
    >
      <div className="absolute top-[10%] left-1/2 w-full max-w-xl -translate-x-1/2 px-4">
        <div
          className="rounded-lg border border-neutral-200 bg-white shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <Command
            shouldFilter={false}
            className="rounded-lg **:[[cmdk-input-wrapper]]:border-b-0"
          >
            <CommandInput
              ref={inputRef}
              placeholder="Search documentation..."
              value={query}
              onValueChange={setQuery}
            />
            <div className="border-t border-neutral-100" />
            <CommandList className="max-h-100 px-1">
              {isLoading && (
                <div className="py-6 text-center text-sm text-neutral-500">
                  Searching...
                </div>
              )}
              {!isLoading && query && results.length === 0 && (
                <CommandEmpty>No results found.</CommandEmpty>
              )}
              {!isLoading && results.length > 0 && (
                <CommandGroup
                  heading="Documentation"
                  className="**:[[cmdk-group-heading]]:px-2 **:[[cmdk-group-heading]]:py-2 **:[[cmdk-group-heading]]:text-xs **:[[cmdk-group-heading]]:font-semibold **:[[cmdk-group-heading]]:tracking-wider **:[[cmdk-group-heading]]:text-neutral-500 **:[[cmdk-group-heading]]:uppercase"
                >
                  {results.map((result, index) => (
                    <CommandItem
                      key={`${result.url}-${index}`}
                      value={result.url}
                      onSelect={() => handleSelect(result.url)}
                      className="flex cursor-pointer items-start gap-3 rounded-md px-2 py-3 hover:bg-neutral-100 data-[selected=true]:bg-neutral-100"
                    >
                      <FileText
                        size={16}
                        className="mt-0.5 shrink-0 text-neutral-400"
                      />
                      <div className="flex min-w-0 flex-col gap-1">
                        <span className="truncate font-medium text-neutral-900">
                          {result.meta?.title || result.url}
                        </span>
                        {result.excerpt && (
                          <span
                            className="line-clamp-2 text-xs text-neutral-500"
                            dangerouslySetInnerHTML={{
                              __html: result.excerpt,
                            }}
                          />
                        )}
                      </div>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {!query && (
                <div className="py-6 text-center text-sm text-neutral-500">
                  Type to search documentation...
                </div>
              )}
            </CommandList>
          </Command>
        </div>
      </div>
    </div>,
    document.body,
  );
}

export function Search() {
  return (
    <SearchPaletteProvider>
      <SearchTrigger />
    </SearchPaletteProvider>
  );
}
