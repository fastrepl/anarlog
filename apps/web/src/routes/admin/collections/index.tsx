import { MDXContent } from "@content-collections/mdx/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { allArticles } from "content-collections";
import {
  AlertTriangleIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClipboardIcon,
  CopyIcon,
  CopyPlusIcon,
  EyeIcon,
  FilePlusIcon,
  FileTextIcon,
  FileWarningIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  GithubIcon,
  ImageIcon,
  type LucideIcon,
  PencilIcon,
  PinIcon,
  PinOffIcon,
  PlusIcon,
  RefreshCwIcon,
  SaveIcon,
  ScissorsIcon,
  SearchIcon,
  SquareArrowOutUpRightIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import {
  ScrollFadeOverlay,
  useScrollFade,
} from "@hypr/ui/components/ui/scroll-fade";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import BlogEditor, {
  type TiptapEditor,
  useBlogEditor,
} from "@/components/admin/blog-editor";
import { MediaSelectorModal } from "@/components/admin/media-selector-modal";
import { defaultMDXComponents } from "@/components/mdx";
import { fetchGitHubCredentials } from "@/functions/admin";
import {
  uploadBlogImageFile,
  uploadInlineMarkdownImages,
} from "@/functions/media-upload";
import { AUTHORS } from "@/lib/team";

interface ContentItem {
  name: string;
  path: string;
  slug: string;
  type: "file";
  collection: string;
  branch?: string;
  isDraft?: boolean;
}

interface DraftArticle {
  name: string;
  path: string;
  slug: string;
  branch: string;
  meta_title?: string;
  author?: string;
  date?: string;
}

interface CollectionInfo {
  name: string;
  label: string;
  items: ContentItem[];
}

const DRAFT_ARTICLES_QUERY_KEY = ["draftArticles"];

interface Tab {
  id: string;
  type: "collection" | "file";
  name: string;
  path: string;
  branch?: string;
  pinned: boolean;
  active: boolean;
}

interface ClipboardItem {
  item: ContentItem;
  operation: "cut" | "copy";
}

interface EditingItem {
  collectionName: string;
  type: "new-file" | "new-folder" | "rename";
  itemPath?: string;
  itemName?: string;
}

interface DeleteConfirmation {
  item: ContentItem;
  collectionName: string;
}

interface FileContent {
  content: string;
  mdx: string;
  collection: string;
  slug: string;
  meta_title?: string;
  display_title?: string;
  meta_description?: string;
  author?: string[];
  date?: string;
  coverImage?: string;
  featured?: boolean;
  category?: string;
}

interface ArticleMetadata {
  meta_title: string;
  display_title: string;
  meta_description: string;
  author: string[];
  date: string;
  coverImage: string;
  featured: boolean;
  category: string;
}

interface EditorData {
  content: string;
  metadata: ArticleMetadata;
  hasUnsavedChanges?: boolean;
  autoSaveCountdown?: number | null;
}

type FileEditorHandle = {
  getData: () => EditorData | null;
};

function getEditorMarkdown(editor: TiptapEditor | null, fallback = "") {
  if (!editor?.isInitialized) {
    return fallback;
  }

  return editor.markdown?.serialize(editor.getJSON()) ?? fallback;
}

function getFileContent(path: string): FileContent | undefined {
  const [collection, ...rest] = path.split("/");
  const filePath = rest.join("/");

  if (collection !== "articles") return undefined;

  const a = allArticles.find((a) => a._meta.fileName === filePath);
  if (!a) return undefined;
  return {
    content: a.content,
    mdx: a.mdx,
    collection: "articles",
    slug: a.slug,
    meta_title: a.meta_title,
    display_title: a.display_title,
    meta_description: a.meta_description,
    author: a.author,
    date: a.date,
    coverImage: a.coverImage,
    featured: a.featured,
    category: a.category,
  };
}

function getCollections(draftArticles: DraftArticle[] = []): CollectionInfo[] {
  const sortedArticles = [...allArticles].sort(
    (a, b) => new Date(b.date).getTime() - new Date(a.date).getTime(),
  );

  const publishedItems: ContentItem[] = sortedArticles.map((a) => ({
    name: a._meta.fileName,
    path: `articles/${a._meta.fileName}`,
    slug: a.slug,
    type: "file" as const,
    collection: "articles",
    isDraft: false,
  }));

  const draftItems: ContentItem[] = draftArticles
    .filter((d) => !publishedItems.some((p) => p.slug === d.slug))
    .map((d) => ({
      name: d.name,
      path: d.path,
      slug: d.slug,
      type: "file" as const,
      collection: "articles",
      branch: d.branch,
      isDraft: true,
    }));

  const allItems = [...draftItems, ...publishedItems];

  return [
    {
      name: "articles",
      label: "Articles",
      items: allItems,
    },
  ];
}

export const Route = createFileRoute("/admin/collections/")({
  beforeLoad: async () => {
    if (import.meta.env.DEV) {
      return;
    }

    const { hasCredentials, isValid } = await fetchGitHubCredentials();

    if (!hasCredentials || !isValid) {
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          provider: "github",
          redirect: "/admin/collections/",
          rra: true,
        },
      });
    }
  },
  component: CollectionsPage,
});

async function fetchDraftArticles() {
  const response = await fetch("/api/admin/content/list-drafts", {
    cache: "no-store",
  });

  if (!response.ok) {
    throw new Error("Failed to fetch drafts");
  }

  const data = await response.json();
  return data.drafts as DraftArticle[];
}

function getFileExtension(filename: string): string {
  const parts = filename.split(".");
  return parts.length > 1 ? parts.pop()?.toLowerCase() || "" : "";
}

function CollectionsPage() {
  const queryClient = useQueryClient();

  const { data: draftArticles = [] } = useQuery({
    queryKey: DRAFT_ARTICLES_QUERY_KEY,
    queryFn: fetchDraftArticles,
    staleTime: 30000,
  });

  const collections = useMemo(
    () => getCollections(draftArticles),
    [draftArticles],
  );
  const [searchQuery, setSearchQuery] = useState("");
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [clipboard, setClipboard] = useState<ClipboardItem | null>(null);
  const [isCreatingNewPost, setIsCreatingNewPost] = useState(false);
  const [editingItem, setEditingItem] = useState<EditingItem | null>(null);
  const [deleteConfirmation, setDeleteConfirmation] =
    useState<DeleteConfirmation | null>(null);

  const draftSyncTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const scheduleDraftSync = useCallback(() => {
    if (draftSyncTimerRef.current) {
      clearTimeout(draftSyncTimerRef.current);
    }
    draftSyncTimerRef.current = setTimeout(() => {
      draftSyncTimerRef.current = null;
      void queryClient.refetchQueries({
        queryKey: DRAFT_ARTICLES_QUERY_KEY,
        type: "active",
      });
    }, 5000);
  }, [queryClient]);

  const createMutation = useMutation({
    mutationFn: async (params: {
      folder: string;
      name: string;
      type: "file" | "folder";
    }) => {
      const response = await fetch("/api/admin/content/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      });
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to create");
      }
      return response.json();
    },
    onSuccess: (data, variables) => {
      setEditingItem(null);
      if (data.branch && variables.type === "file") {
        const path = data.path || `${variables.folder}/${variables.name}`;
        const name = path.split("/").pop() || variables.name;
        const slug = name.replace(/\.mdx$/, "");
        queryClient.setQueryData(
          DRAFT_ARTICLES_QUERY_KEY,
          (old: DraftArticle[] = []) => [
            ...old.filter(
              (draft) =>
                draft.branch !== data.branch &&
                draft.path !== path &&
                draft.slug !== slug,
            ),
            {
              name,
              path,
              slug,
              branch: data.branch,
            },
          ],
        );
        openTab("file", name, path, data.branch);
        setIsCreatingNewPost(false);
        scheduleDraftSync();
      } else {
        setIsCreatingNewPost(false);
        void queryClient.invalidateQueries({
          queryKey: DRAFT_ARTICLES_QUERY_KEY,
        });
      }
    },
    onError: (error) => {
      sonnerToast.error("Create failed", {
        description: error.message,
      });
    },
  });

  const renameMutation = useMutation({
    mutationFn: async (params: { fromPath: string; toPath: string }) => {
      const response = await fetch("/api/admin/content/rename", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      });
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to rename");
      }
      return response.json();
    },
    onSuccess: () => {
      setEditingItem(null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (params: { path: string }) => {
      const response = await fetch("/api/admin/content/delete", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      });
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to delete");
      }
      return response.json();
    },
    onSuccess: (_data, variables) => {
      const deletedPath = variables.path;
      setDeleteConfirmation(null);
      setTabs((prev) => {
        const filtered = prev.filter((t) => t.path !== deletedPath);
        if (filtered.length > 0 && !filtered.some((t) => t.active)) {
          return filtered.map((t, i) =>
            i === filtered.length - 1 ? { ...t, active: true } : t,
          );
        }
        return filtered;
      });
      void queryClient.invalidateQueries({
        queryKey: DRAFT_ARTICLES_QUERY_KEY,
      });
    },
  });

  const duplicateMutation = useMutation({
    mutationFn: async (params: {
      sourcePath: string;
      newFilename?: string;
    }) => {
      const response = await fetch("/api/admin/content/duplicate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      });
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to duplicate");
      }
      return response.json();
    },
  });

  const currentTab = tabs.find((t) => t.active);

  const openTab = useCallback(
    (
      type: "collection" | "file",
      name: string,
      path: string,
      branch?: string,
      pinned = false,
    ) => {
      setTabs((prev) => {
        const existingIndex = prev.findIndex(
          (t) => t.type === type && t.path === path && t.branch === branch,
        );
        if (existingIndex !== -1) {
          return prev.map((t, i) => ({ ...t, active: i === existingIndex }));
        }

        const unpinnedIndex = prev.findIndex((t) => !t.pinned);
        const newTab: Tab = {
          id: `${type}-${path}-${branch || "main"}-${Date.now()}`,
          type,
          name,
          path,
          branch,
          pinned,
          active: true,
        };

        if (unpinnedIndex !== -1) {
          return prev.map((t, i) =>
            i === unpinnedIndex ? newTab : { ...t, active: false },
          );
        }

        return [...prev.map((t) => ({ ...t, active: false })), newTab];
      });
    },
    [],
  );

  const closeTab = useCallback((tabId: string) => {
    setTabs((prev) => {
      const index = prev.findIndex((t) => t.id === tabId);
      if (index === -1) return prev;

      const newTabs = prev.filter((t) => t.id !== tabId);
      if (newTabs.length === 0) return [];

      if (prev[index].active) {
        const newActiveIndex = Math.min(index, newTabs.length - 1);
        return newTabs.map((t, i) => ({ ...t, active: i === newActiveIndex }));
      }
      return newTabs;
    });
  }, []);

  const closeOtherTabs = useCallback((tabId: string) => {
    setTabs((prev) => {
      const tab = prev.find((t) => t.id === tabId);
      if (!tab) return prev;
      return [{ ...tab, active: true }];
    });
  }, []);

  const closeAllTabs = useCallback(() => {
    setTabs([]);
  }, []);

  const selectTab = useCallback((tabId: string) => {
    setTabs((prev) => prev.map((t) => ({ ...t, active: t.id === tabId })));
  }, []);

  const pinTab = useCallback((tabId: string) => {
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, pinned: !t.pinned } : t)),
    );
  }, []);

  const reorderTabs = useCallback((newTabs: Tab[]) => {
    setTabs(newTabs);
  }, []);

  const filterCollections = (
    items: CollectionInfo[],
    query: string,
  ): CollectionInfo[] => {
    if (!query) return items;
    const lowerQuery = query.toLowerCase();

    return items.filter(
      (item) =>
        item.label.toLowerCase().includes(lowerQuery) ||
        item.name.toLowerCase().includes(lowerQuery) ||
        item.items.some((i) => i.name.toLowerCase().includes(lowerQuery)),
    );
  };

  const filteredCollections = filterCollections(collections, searchQuery);

  const currentCollection =
    currentTab?.type === "collection"
      ? collections.find((c) => c.name === currentTab.path)
      : null;

  const filteredItems =
    currentCollection?.items.filter((item) => {
      return (
        searchQuery === "" ||
        item.name.toLowerCase().includes(searchQuery.toLowerCase())
      );
    }) || [];

  return (
    <ResizablePanelGroup direction="horizontal" className="h-full">
      <ResizablePanel defaultSize={20} minSize={15} maxSize={30}>
        <Sidebar
          collections={filteredCollections}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          onFileClick={(item) =>
            openTab("file", item.name, item.path, item.branch)
          }
          clipboard={clipboard}
          onClipboardChange={setClipboard}
          onNewPostClick={() => setIsCreatingNewPost(true)}
          isCreatingNewPost={isCreatingNewPost}
          onCreateNewPost={(slug) => {
            createMutation.mutate({
              folder: "articles",
              name: `${slug}.mdx`,
              type: "file",
            });
          }}
          onCancelNewPost={() => setIsCreatingNewPost(false)}
          editingItem={editingItem}
          onEditingItemChange={setEditingItem}
          onRenameItem={(fromPath, toPath) =>
            renameMutation.mutate({ fromPath, toPath })
          }
          onDeleteItem={(item, collectionName) =>
            setDeleteConfirmation({ item, collectionName })
          }
          onDuplicateItem={(sourcePath) =>
            duplicateMutation.mutate({ sourcePath })
          }
          isLoading={
            createMutation.isPending ||
            renameMutation.isPending ||
            deleteMutation.isPending ||
            duplicateMutation.isPending
          }
          selectedPath={currentTab?.type === "file" ? currentTab.path : null}
        />
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel defaultSize={80} minSize={50}>
        <div className="flex h-full flex-col">
          <ContentPanel
            tabs={tabs}
            currentTab={currentTab}
            onSelectTab={selectTab}
            onCloseTab={closeTab}
            onCloseOtherTabs={closeOtherTabs}
            onCloseAllTabs={closeAllTabs}
            onPinTab={pinTab}
            onReorderTabs={reorderTabs}
            filteredItems={filteredItems}
            onFileClick={(item) =>
              openTab("file", item.name, item.path, item.branch)
            }
            onRenameFile={(fromPath, toPath) =>
              renameMutation.mutate({ fromPath, toPath })
            }
            onDeleteFile={(path) =>
              setDeleteConfirmation({
                item: {
                  name: path.split("/").pop() || path,
                  path,
                  slug: (path.split("/").pop() || "").replace(/\.mdx$/, ""),
                  type: "file",
                  collection: path.split("/")[0] || "articles",
                },
                collectionName: path.split("/")[0] || "articles",
              })
            }
            isDeleting={deleteMutation.isPending}
          />
        </div>
      </ResizablePanel>
      <Dialog
        open={deleteConfirmation !== null}
        onOpenChange={(open) => !open && setDeleteConfirmation(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Delete File</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-4">
            <p className="text-sm text-neutral-600">
              Are you sure you want to delete{" "}
              <span className="font-medium text-neutral-900">
                {deleteConfirmation?.item.name}
              </span>
              ? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setDeleteConfirmation(null)}
                className="rounded px-3 py-1.5 text-sm text-neutral-600 transition-colors hover:bg-neutral-100"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => {
                  if (deleteConfirmation) {
                    deleteMutation.mutate({
                      path: deleteConfirmation.item.path,
                    });
                  }
                }}
                disabled={deleteMutation.isPending}
                className="flex items-center gap-2 rounded bg-red-600 px-3 py-1.5 text-sm text-white transition-colors hover:bg-red-700 disabled:opacity-50"
              >
                {deleteMutation.isPending && (
                  <Spinner size={14} color="white" />
                )}
                {deleteMutation.isPending ? "Deleting..." : "Delete"}
              </button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </ResizablePanelGroup>
  );
}

function Sidebar({
  collections,
  searchQuery,
  onSearchChange,
  onFileClick,
  clipboard,
  onClipboardChange,
  onNewPostClick,
  isCreatingNewPost,
  onCreateNewPost,
  onCancelNewPost,
  editingItem,
  onEditingItemChange,
  onRenameItem,
  onDeleteItem,
  onDuplicateItem,
  isLoading,
  selectedPath,
}: {
  collections: CollectionInfo[];
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onFileClick: (item: ContentItem) => void;
  clipboard: ClipboardItem | null;
  onClipboardChange: (item: ClipboardItem | null) => void;
  onNewPostClick: () => void;
  isCreatingNewPost: boolean;
  onCreateNewPost: (slug: string) => void;
  onCancelNewPost: () => void;
  editingItem: EditingItem | null;
  onEditingItemChange: (item: EditingItem | null) => void;
  onRenameItem: (fromPath: string, toPath: string) => void;
  onDeleteItem: (item: ContentItem, collectionName: string) => void;
  onDuplicateItem: (sourcePath: string) => void;
  isLoading: boolean;
  selectedPath: string | null;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(scrollRef, "vertical", [
    collections,
  ]);

  return (
    <div className="flex h-full min-h-0 flex-col border-r border-neutral-200 bg-white">
      <div className="flex h-10 items-center border-b border-neutral-200 pr-2 pl-4">
        <div className="relative flex w-full items-center gap-1.5">
          <SearchIcon className="size-4 shrink-0 text-neutral-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Search..."
            className={cn([
              "w-full py-1 text-sm",
              "bg-transparent",
              "focus:outline-hidden",
              "placeholder:text-neutral-400",
            ])}
          />
        </div>
      </div>

      <div className="relative min-h-0 flex-1">
        {!atStart && <ScrollFadeOverlay position="top" />}
        {!atEnd && <ScrollFadeOverlay position="bottom" />}
        <div ref={scrollRef} className="h-full overflow-y-auto">
          {isCreatingNewPost && (
            <NewPostInlineInput
              existingSlugs={
                collections[0]?.items.map((item) => item.slug) || []
              }
              onSubmit={onCreateNewPost}
              onCancel={onCancelNewPost}
              isLoading={isLoading}
            />
          )}
          {collections[0]?.items.map((item) => (
            <FileItemSidebar
              key={item.path}
              item={item}
              onClick={() => onFileClick(item)}
              clipboard={clipboard}
              onClipboardChange={onClipboardChange}
              editingItem={editingItem}
              onEditingItemChange={onEditingItemChange}
              onRenameItem={onRenameItem}
              onDeleteItem={onDeleteItem}
              onDuplicateItem={onDuplicateItem}
              collectionName="articles"
              isLoading={isLoading}
              isSelected={selectedPath === item.path}
            />
          ))}
        </div>
      </div>

      <div className="p-3">
        <button
          onClick={onNewPostClick}
          disabled={isCreatingNewPost}
          className={cn([
            "flex h-9 w-full items-center justify-center gap-2 rounded-full text-sm font-medium",
            "border border-neutral-200 bg-linear-to-b from-white to-neutral-100 text-neutral-700",
            "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            "disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100 disabled:hover:shadow-xs",
          ])}
        >
          <PlusIcon className="size-4" />
          New Post
        </button>
      </div>
    </div>
  );
}

function FileItemSidebar({
  item,
  onClick,
  clipboard,
  onClipboardChange,
  editingItem,
  onEditingItemChange,
  onRenameItem,
  onDeleteItem,
  onDuplicateItem,
  collectionName,
  isLoading,
  isSelected,
}: {
  item: ContentItem;
  onClick: () => void;
  clipboard: ClipboardItem | null;
  onClipboardChange: (item: ClipboardItem | null) => void;
  editingItem: EditingItem | null;
  onEditingItemChange: (item: EditingItem | null) => void;
  onRenameItem: (fromPath: string, toPath: string) => void;
  onDeleteItem: (item: ContentItem, collectionName: string) => void;
  onDuplicateItem: (sourcePath: string) => void;
  collectionName: string;
  isLoading: boolean;
  isSelected: boolean;
}) {
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const closeContextMenu = () => setContextMenu(null);

  const isRenaming =
    editingItem?.type === "rename" && editingItem?.itemPath === item.path;

  const isCut =
    clipboard?.operation === "cut" && clipboard?.item.path === item.path;

  if (isRenaming) {
    return (
      <InlineInput
        type="file"
        defaultValue={item.name.replace(/\.mdx$/, "")}
        onSubmit={(newName) => {
          const newPath = `${collectionName}/${newName}.mdx`;
          onRenameItem(item.path, newPath);
        }}
        onCancel={() => onEditingItemChange(null)}
        isLoading={isLoading}
      />
    );
  }

  return (
    <div
      className={cn([
        "flex cursor-pointer items-center gap-1.5 py-1.5 pr-2 pl-4 text-sm",
        "transition-colors hover:bg-neutral-50",
        isCut && "opacity-50",
        (isSelected || contextMenu) && "bg-neutral-100",
      ])}
      onClick={onClick}
      onContextMenu={handleContextMenu}
    >
      <FileTextIcon className="size-4 shrink-0 text-neutral-400" />
      <span className="truncate text-neutral-600">
        {item.name.replace(/\.mdx$/, "")}
      </span>

      {item.isDraft && (
        <span className="shrink-0 rounded bg-amber-100 px-1.5 py-0.5 text-[10px] font-medium text-amber-700">
          Draft
        </span>
      )}

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={closeContextMenu}
          isFolder={false}
          canPaste={clipboard !== null}
          onOpenInNewTab={() => {
            onClick();
            closeContextMenu();
          }}
          onCut={() => {
            onClipboardChange({ item, operation: "cut" });
            closeContextMenu();
          }}
          onCopy={() => {
            onClipboardChange({ item, operation: "copy" });
            closeContextMenu();
          }}
          onDuplicate={() => {
            onDuplicateItem(item.path);
            closeContextMenu();
          }}
          onRename={() => {
            onEditingItemChange({
              collectionName,
              type: "rename",
              itemPath: item.path,
              itemName: item.name,
            });
            closeContextMenu();
          }}
          onDelete={() => {
            onDeleteItem(item, collectionName);
            closeContextMenu();
          }}
        />
      )}
    </div>
  );
}

function InlineInput({
  type,
  defaultValue = "",
  onSubmit,
  onCancel,
  isLoading,
}: {
  type: "file" | "folder";
  defaultValue?: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
  isLoading: boolean;
}) {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && value.trim()) {
      onSubmit(value.trim());
    } else if (e.key === "Escape") {
      onCancel();
    }
  };

  const handleBlur = () => {
    if (value.trim()) {
      onSubmit(value.trim());
    } else {
      onCancel();
    }
  };

  return (
    <div
      className={cn([
        "flex items-center gap-1.5 py-1.5 pr-2 pl-4 text-sm",
        "bg-neutral-100",
      ])}
    >
      {type === "file" ? (
        <FileTextIcon className="size-4 shrink-0 text-neutral-400" />
      ) : (
        <FolderIcon className="size-4 shrink-0 text-neutral-400" />
      )}
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        disabled={isLoading}
        placeholder={type === "file" ? "filename" : "folder name"}
        className={cn([
          "flex-1 bg-transparent text-sm outline-hidden",
          "text-neutral-600 placeholder:text-neutral-400",
        ])}
      />
    </div>
  );
}

function NewPostInlineInput({
  existingSlugs,
  onSubmit,
  onCancel,
  isLoading,
}: {
  existingSlugs: string[];
  onSubmit: (slug: string) => void;
  onCancel: () => void;
  isLoading: boolean;
}) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const hasSubmittedRef = useRef(false);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!isLoading) {
      hasSubmittedRef.current = false;
    }
  }, [isLoading]);

  const validateSlug = (slug: string): string | null => {
    if (!slug.trim()) {
      return "Slug cannot be empty";
    }

    // Check if slug already exists
    if (existingSlugs.includes(slug.toLowerCase())) {
      return "Slug already exists";
    }

    // Validate slug format: lowercase, alphanumeric, hyphens only
    const slugRegex = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
    if (!slugRegex.test(slug)) {
      return "Slug must be lowercase, alphanumeric, and hyphens only";
    }

    return null;
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      const slug = value.trim().toLowerCase();
      const validationError = validateSlug(slug);
      if (validationError) {
        setError(validationError);
      } else if (!hasSubmittedRef.current) {
        hasSubmittedRef.current = true;
        setError(null);
        onSubmit(slug);
      }
    } else if (e.key === "Escape") {
      onCancel();
    }
  };

  const handleBlur = () => {
    if (!value.trim()) {
      onCancel();
      return;
    }

    const slug = value.trim().toLowerCase();
    const validationError = validateSlug(slug);
    if (validationError) {
      setError(validationError);
      // Keep focus if there's an error
      setTimeout(() => inputRef.current?.focus(), 0);
    } else if (!hasSubmittedRef.current) {
      hasSubmittedRef.current = true;
      setError(null);
      onSubmit(slug);
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value.toLowerCase();
    setValue(newValue);
    // Clear error on change
    if (error) {
      setError(null);
    }
  };

  return (
    <div>
      <div
        className={cn([
          "flex items-center gap-1.5 py-1.5 pr-2 pl-4 text-sm",
          error ? "bg-red-50" : "bg-neutral-100",
        ])}
      >
        <FileTextIcon className="size-4 shrink-0 text-neutral-400" />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          disabled={isLoading}
          placeholder="enter-slug-here"
          className={cn([
            "flex-1 bg-transparent text-sm outline-hidden",
            error ? "text-red-700" : "text-neutral-600",
            "placeholder:text-neutral-400",
          ])}
        />
      </div>
      {error && (
        <div className="bg-red-50 px-4 py-1 text-xs text-red-600">{error}</div>
      )}
    </div>
  );
}

function ContextMenu({
  x,
  y,
  onClose,
  isFolder,
  canPaste,
  onOpenInNewTab,
  onNewFile,
  onNewFolder,
  onCut,
  onCopy,
  onDuplicate,
  onPaste,
  onRename,
  onDelete,
}: {
  x: number;
  y: number;
  onClose: () => void;
  isFolder: boolean;
  canPaste: boolean;
  onOpenInNewTab: () => void;
  onNewFile?: () => void;
  onNewFolder?: () => void;
  onCut?: () => void;
  onCopy?: () => void;
  onDuplicate?: () => void;
  onPaste?: () => void;
  onRename?: () => void;
  onDelete?: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  return (
    <>
      <div
        className="fixed inset-0 z-40"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          onClose();
        }}
      />
      <div
        ref={menuRef}
        className={cn([
          "fixed z-50 min-w-40 py-1",
          "rounded-xs border border-neutral-200 bg-white shadow-lg",
        ])}
        style={{ left: x, top: y }}
      >
        <ContextMenuItem
          icon={SquareArrowOutUpRightIcon}
          label="Open in new tab"
          onClick={() => {
            onOpenInNewTab();
            onClose();
          }}
        />

        {isFolder && (
          <>
            <div className="my-1 border-t border-neutral-200" />
            {onNewFile && (
              <ContextMenuItem
                icon={FilePlusIcon}
                label="New file"
                onClick={onNewFile}
              />
            )}
            {onNewFolder && (
              <ContextMenuItem
                icon={FolderPlusIcon}
                label="New folder"
                onClick={onNewFolder}
              />
            )}
          </>
        )}

        {!isFolder && (
          <>
            <div className="my-1 border-t border-neutral-200" />

            {onCut && (
              <ContextMenuItem
                icon={ScissorsIcon}
                label="Cut"
                onClick={onCut}
              />
            )}
            {onCopy && (
              <ContextMenuItem icon={CopyIcon} label="Copy" onClick={onCopy} />
            )}
            {onDuplicate && (
              <ContextMenuItem
                icon={CopyPlusIcon}
                label="Duplicate"
                onClick={onDuplicate}
              />
            )}

            <div className="my-1 border-t border-neutral-200" />

            {onRename && (
              <ContextMenuItem
                icon={PencilIcon}
                label="Rename"
                onClick={onRename}
              />
            )}
            {onDelete && (
              <ContextMenuItem
                icon={Trash2Icon}
                label="Delete"
                onClick={onDelete}
                danger
              />
            )}
          </>
        )}

        {isFolder && onPaste && (
          <>
            <div className="my-1 border-t border-neutral-200" />
            <ContextMenuItem
              icon={ClipboardIcon}
              label="Paste"
              onClick={onPaste}
              disabled={!canPaste}
            />
          </>
        )}
      </div>
    </>
  );
}

function ContextMenuItem({
  icon: Icon,
  label,
  onClick,
  disabled,
  danger,
}: {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn([
        "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm",
        "transition-colors hover:bg-neutral-100",
        disabled && "cursor-not-allowed opacity-40 hover:bg-transparent",
        danger && "text-red-600 hover:bg-red-50",
      ])}
    >
      <Icon className="size-4" />
      {label}
    </button>
  );
}

function ContentPanel({
  tabs,
  currentTab,
  onSelectTab,
  onCloseTab,
  onCloseOtherTabs,
  onCloseAllTabs,
  onPinTab,
  onReorderTabs,
  filteredItems,
  onFileClick,
  onRenameFile,
  onDeleteFile,
  isDeleting,
}: {
  tabs: Tab[];
  currentTab: Tab | undefined;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onCloseOtherTabs: (tabId: string) => void;
  onCloseAllTabs: () => void;
  onPinTab: (tabId: string) => void;
  onReorderTabs: (tabs: Tab[]) => void;
  filteredItems: ContentItem[];
  onFileClick: (item: ContentItem) => void;
  onRenameFile: (fromPath: string, toPath: string) => void;
  onDeleteFile: (path: string) => void;
  isDeleting: boolean;
}) {
  const [isPreviewMode, setIsPreviewMode] = useState(false);
  const [editorData, setEditorData] = useState<EditorData | null>(null);
  const fileEditorRef = useRef<FileEditorHandle | null>(null);
  const queryClient = useQueryClient();

  const getCurrentEditorData = useCallback(
    () => fileEditorRef.current?.getData() ?? editorData,
    [editorData],
  );

  const saveArticle = useCallback(
    async (params: {
      path: string;
      content: string;
      metadata: ArticleMetadata;
      branch?: string;
      isAutoSave?: boolean;
    }) => {
      const processedContent = await uploadInlineMarkdownImages({
        content: params.content,
        path: params.path,
      });

      const response = await fetch("/api/admin/content/save", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ...params,
          content: processedContent,
        }),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to save");
      }

      return response.json();
    },
    [],
  );

  const { mutate: saveContent, isPending: isSaving } = useMutation({
    mutationFn: saveArticle,
    onSuccess: (data, variables) => {
      if (data.branchName) {
        queryClient.invalidateQueries({
          queryKey: ["pendingPR", variables.path],
        });
      }
    },
    onError: (error) => {
      sonnerToast.error("Save failed", {
        description: error.message,
      });
    },
  });

  const handleSave = useCallback(
    (options?: { isAutoSave?: boolean }) => {
      const currentEditorData = getCurrentEditorData();

      if (currentTab?.type === "file" && currentEditorData) {
        saveContent({
          path: currentTab.path,
          content: currentEditorData.content,
          metadata: currentEditorData.metadata,
          branch: currentTab.branch,
          isAutoSave: options?.isAutoSave,
        });
      }
    },
    [currentTab, getCurrentEditorData, saveContent],
  );

  const { data: pendingPRData } = useQuery({
    queryKey: ["pendingPR", currentTab?.path],
    queryFn: async () => {
      const params = new URLSearchParams({ path: currentTab!.path });
      const response = await fetch(`/api/admin/content/pending-pr?${params}`);
      if (!response.ok) {
        return { hasPendingPR: false };
      }
      return response.json() as Promise<{
        hasPendingPR: boolean;
        prNumber?: number;
        prUrl?: string;
        branchName?: string;
      }>;
    },
    enabled:
      !!currentTab?.path &&
      currentTab?.type === "file" &&
      currentTab.path.startsWith("articles/"),
    staleTime: 60000,
  });

  const { mutateAsync: publish, isPending: isPublishing } = useMutation({
    mutationFn: async (params: {
      path: string;
      content: string;
      metadata: ArticleMetadata;
      branch?: string;
    }) => {
      const saveResult = await saveArticle(params);

      if (saveResult.prUrl) {
        return { prUrl: saveResult.prUrl as string };
      }

      let branchName = saveResult.branchName || params.branch;

      if (!branchName) {
        const prParams = new URLSearchParams({ path: params.path });
        const prResponse = await fetch(
          `/api/admin/content/pending-pr?${prParams}`,
        );
        if (prResponse.ok) {
          const prData = await prResponse.json();
          if (prData.hasPendingPR && prData.prUrl) {
            return { prUrl: prData.prUrl as string };
          }
          if (prData.branchName) {
            branchName = prData.branchName;
          }
        }
      }

      if (!branchName) {
        throw new Error("No branch available for publishing");
      }

      const publishResponse = await fetch("/api/admin/content/publish", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          path: params.path,
          branch: branchName,
          metadata: params.metadata,
        }),
      });
      if (!publishResponse.ok) {
        const error = await publishResponse.json();
        throw new Error(error.error || "Failed to publish");
      }
      const publishResult = await publishResponse.json();
      return { prUrl: publishResult.prUrl as string | undefined };
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: ["pendingPR", variables.path],
      });
    },
    onError: (error) => {
      sonnerToast.error("Publish failed", {
        description: error.message,
      });
    },
  });

  const handlePublish = useCallback(async () => {
    const currentEditorData = getCurrentEditorData();

    if (!currentTab || !currentEditorData) return;

    const popup = window.open("", "_blank");

    try {
      const data = await publish({
        path: currentTab.path,
        content: currentEditorData.content,
        metadata: currentEditorData.metadata,
        branch: currentTab.branch,
      });

      if (data.prUrl) {
        if (popup) {
          popup.location.href = data.prUrl;
          return;
        }

        sonnerToast.success("PR created", {
          description: "Pop-up was blocked by your browser.",
          action: {
            label: "Open PR",
            onClick: () => window.open(data.prUrl, "_blank"),
          },
        });
        return;
      }

      popup?.close();
    } catch {
      popup?.close();
    }
  }, [currentTab, getCurrentEditorData, publish]);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {currentTab ? (
        <div className="flex min-h-0 flex-1 flex-col">
          <EditorHeader
            tabs={tabs}
            currentTab={currentTab}
            onSelectTab={onSelectTab}
            onCloseTab={onCloseTab}
            onCloseOtherTabs={onCloseOtherTabs}
            onCloseAllTabs={onCloseAllTabs}
            onPinTab={onPinTab}
            onReorderTabs={onReorderTabs}
            isPreviewMode={isPreviewMode}
            onTogglePreview={() => setIsPreviewMode(!isPreviewMode)}
            onSave={handleSave}
            isSaving={isSaving}
            onPublish={handlePublish}
            isPublishing={isPublishing}
            hasPendingPR={pendingPRData?.hasPendingPR}
            onRenameFile={(newSlug) => {
              const pathParts = currentTab.path.split("/");
              pathParts[pathParts.length - 1] = `${newSlug}.mdx`;
              const newPath = pathParts.join("/");
              onRenameFile(currentTab.path, newPath);
            }}
            onDelete={() => onDeleteFile(currentTab.path)}
            isDeleting={isDeleting}
            hasUnsavedChanges={editorData?.hasUnsavedChanges}
            autoSaveCountdown={editorData?.autoSaveCountdown}
          />
          {currentTab.type === "collection" ? (
            <FileList filteredItems={filteredItems} onFileClick={onFileClick} />
          ) : (
            <FileEditor
              ref={fileEditorRef}
              filePath={currentTab.path}
              branch={currentTab.branch}
              isPreviewMode={isPreviewMode}
              onDataChange={setEditorData}
              onSave={handleSave}
              isSaving={isSaving}
            />
          )}
        </div>
      ) : (
        <div className="flex flex-1 flex-col">
          <div className="h-10 border-b border-neutral-200" />
          <EmptyState
            icon={FolderOpenIcon}
            message="Double-click a collection or click a file to open"
          />
        </div>
      )}
    </div>
  );
}

function EditorHeader({
  tabs,
  currentTab,
  onSelectTab,
  onCloseTab,
  onCloseOtherTabs,
  onCloseAllTabs,
  onPinTab,
  onReorderTabs,
  isPreviewMode,
  onTogglePreview,
  onSave,
  isSaving,
  onPublish,
  isPublishing,
  hasPendingPR,
  onRenameFile,
  onDelete,
  isDeleting,
  hasUnsavedChanges,
  autoSaveCountdown,
}: {
  tabs: Tab[];
  currentTab: Tab;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onCloseOtherTabs: (tabId: string) => void;
  onCloseAllTabs: () => void;
  onPinTab: (tabId: string) => void;
  onReorderTabs: (tabs: Tab[]) => void;
  isPreviewMode: boolean;
  onTogglePreview: () => void;
  onSave: () => void;
  isSaving: boolean;
  onPublish?: () => void;
  isPublishing?: boolean;
  hasPendingPR?: boolean;
  onRenameFile?: (newSlug: string) => void;
  onDelete?: () => void;
  isDeleting?: boolean;
  hasUnsavedChanges?: boolean;
  autoSaveCountdown?: number | null;
}) {
  const [isEditingSlug, setIsEditingSlug] = useState(false);
  const [slugValue, setSlugValue] = useState("");
  const slugInputRef = useRef<HTMLInputElement>(null);
  const breadcrumbs = currentTab.path.split("/");
  const currentSlug =
    breadcrumbs[breadcrumbs.length - 1]?.replace(/\.mdx$/, "") || "";

  const handleSlugClick = () => {
    if (currentTab.type === "file" && onRenameFile) {
      setSlugValue(currentSlug);
      setIsEditingSlug(true);
      setTimeout(() => slugInputRef.current?.focus(), 0);
    }
  };

  const handleSlugSubmit = () => {
    const trimmedSlug = slugValue.trim();
    if (trimmedSlug && trimmedSlug !== currentSlug && onRenameFile) {
      onRenameFile(trimmedSlug);
    }
    setIsEditingSlug(false);
  };

  const handleSlugKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleSlugSubmit();
    } else if (e.key === "Escape") {
      setIsEditingSlug(false);
    }
  };

  return (
    <div>
      <div className="flex items-end">
        <TabBar
          tabs={tabs}
          onSelectTab={onSelectTab}
          onCloseTab={onCloseTab}
          onCloseOtherTabs={onCloseOtherTabs}
          onCloseAllTabs={onCloseAllTabs}
          onPinTab={onPinTab}
          onReorderTabs={onReorderTabs}
        />
        <div className="flex-1 border-b border-neutral-200" />
      </div>

      <div className="flex h-10 items-center justify-between border-b border-neutral-200 px-4">
        <div className="flex items-center gap-1 text-sm text-neutral-500">
          {breadcrumbs.map((crumb, index) => (
            <span key={index} className="flex items-center gap-1">
              {index > 0 && (
                <ChevronRightIcon className="size-4 text-neutral-300" />
              )}
              {index === breadcrumbs.length - 1 &&
              currentTab.type === "file" ? (
                isEditingSlug ? (
                  <input
                    ref={slugInputRef}
                    type="text"
                    value={slugValue}
                    onChange={(e) => setSlugValue(e.target.value)}
                    onBlur={handleSlugSubmit}
                    onKeyDown={handleSlugKeyDown}
                    className="bg-transparent font-medium text-neutral-700 outline-none"
                  />
                ) : (
                  <span
                    onClick={handleSlugClick}
                    className="cursor-text font-medium text-neutral-700 hover:text-neutral-900"
                  >
                    {crumb.replace(/\.mdx$/, "")}
                  </span>
                )
              ) : (
                <span
                  className={cn([
                    index === breadcrumbs.length - 1
                      ? "font-medium text-neutral-700"
                      : "cursor-pointer hover:text-neutral-700",
                  ])}
                >
                  {crumb.replace(/\.mdx$/, "")}
                </span>
              )}
            </span>
          ))}
        </div>

        {currentTab.type === "file" && (
          <div className="flex items-center gap-1">
            {onDelete && (
              <button
                onClick={onDelete}
                disabled={isDeleting}
                className={cn([
                  "flex cursor-pointer items-center gap-1.5 rounded-xs px-2 py-1.5 font-mono text-xs font-medium transition-colors",
                  "text-red-600 hover:bg-red-50",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                ])}
                title="Delete"
              >
                {isDeleting ? (
                  <Spinner size={16} color="currentColor" />
                ) : (
                  <Trash2Icon className="size-4" />
                )}
              </button>
            )}
            <button
              onClick={onTogglePreview}
              className={cn([
                "flex cursor-pointer items-center gap-1.5 rounded-xs px-2 py-1.5 font-mono text-xs font-medium transition-colors",
                isPreviewMode
                  ? "text-neutral-700"
                  : "text-neutral-400 hover:text-neutral-600",
              ])}
              title={isPreviewMode ? "Edit mode" : "Preview mode"}
            >
              {isPreviewMode ? (
                <>
                  <PencilIcon className="size-4" />
                  Edit
                </>
              ) : (
                <>
                  <EyeIcon className="size-4" />
                  Preview
                </>
              )}
            </button>
            <button
              onClick={onSave}
              disabled={isSaving || !hasUnsavedChanges}
              className={cn([
                "flex cursor-pointer items-center gap-1.5 rounded-xs px-2 py-1.5 font-mono text-xs font-medium transition-colors",
                "bg-neutral-900 text-white hover:bg-neutral-800",
                "disabled:cursor-not-allowed disabled:opacity-50",
              ])}
              title="Save (⌘S)"
            >
              {isSaving ? (
                <Spinner size={16} color="white" />
              ) : (
                <SaveIcon className="size-4" />
              )}
              Save
              {autoSaveCountdown !== null &&
                autoSaveCountdown !== undefined &&
                hasUnsavedChanges && (
                  <span className="ml-1 text-neutral-400">
                    ({autoSaveCountdown}s)
                  </span>
                )}
            </button>
            {onPublish && (
              <button
                onClick={onPublish}
                disabled={isPublishing}
                className={cn([
                  "flex cursor-pointer items-center gap-1.5 rounded-xs px-2 py-1.5 font-mono text-xs font-medium transition-colors",
                  hasPendingPR
                    ? "bg-amber-600 text-white hover:bg-amber-700"
                    : "bg-blue-600 text-white hover:bg-blue-700",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                ])}
                title={hasPendingPR ? "View existing PR" : "Create PR"}
              >
                {isPublishing ? (
                  <Spinner size={16} color="white" />
                ) : (
                  <SquareArrowOutUpRightIcon className="size-4" />
                )}
                {hasPendingPR ? "View PR" : "Publish"}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function TabBar({
  tabs,
  onSelectTab,
  onCloseTab,
  onCloseOtherTabs,
  onCloseAllTabs,
  onPinTab,
  onReorderTabs,
}: {
  tabs: Tab[];
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onCloseOtherTabs: (tabId: string) => void;
  onCloseAllTabs: () => void;
  onPinTab: (tabId: string) => void;
  onReorderTabs: (tabs: Tab[]) => void;
}) {
  if (tabs.length === 0) {
    return null;
  }

  return (
    <div className="flex items-end overflow-x-auto">
      <Reorder.Group
        as="div"
        axis="x"
        values={tabs}
        onReorder={onReorderTabs}
        className="flex items-end"
      >
        {tabs.map((tab) => (
          <Reorder.Item key={tab.id} value={tab} as="div">
            <TabItem
              tab={tab}
              onSelect={() => onSelectTab(tab.id)}
              onClose={() => onCloseTab(tab.id)}
              onCloseOthers={() => onCloseOtherTabs(tab.id)}
              onCloseAll={onCloseAllTabs}
              onPin={() => onPinTab(tab.id)}
            />
          </Reorder.Item>
        ))}
      </Reorder.Group>
    </div>
  );
}

function TabItem({
  tab,
  onSelect,
  onClose,
  onCloseOthers,
  onCloseAll,
  onPin,
}: {
  tab: Tab;
  onSelect: () => void;
  onClose: () => void;
  onCloseOthers: () => void;
  onCloseAll: () => void;
  onPin: () => void;
}) {
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const handleDoubleClick = () => {
    if (!tab.pinned) {
      onPin();
    }
  };

  const handleAuxClick = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault();
      onClose();
    }
  };

  return (
    <>
      <div
        className={cn([
          "flex h-10 cursor-pointer items-center gap-2 px-3 text-sm transition-colors",
          "border-r border-b border-neutral-200",
          tab.active
            ? "border-b-transparent bg-white text-neutral-900"
            : "bg-neutral-50 text-neutral-600 hover:bg-neutral-100",
        ])}
        onClick={onSelect}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        onAuxClick={handleAuxClick}
      >
        {tab.type === "collection" ? (
          <FolderIcon className="size-4 text-neutral-400" />
        ) : (
          <FileTextIcon className="size-4 text-neutral-400" />
        )}
        <span className={cn(["max-w-30 truncate", !tab.pinned && "italic"])}>
          {tab.name.replace(/\.mdx$/, "")}
        </span>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          className="rounded p-0.5 transition-colors hover:bg-neutral-200"
        >
          <XIcon className="size-3 text-neutral-500" />
        </button>
      </div>

      {contextMenu && (
        <TabContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onCloseTab={onClose}
          onCloseOthers={onCloseOthers}
          onCloseAll={onCloseAll}
          onPinTab={onPin}
          isPinned={tab.pinned}
        />
      )}
    </>
  );
}

function TabContextMenu({
  x,
  y,
  onClose,
  onCloseTab,
  onCloseOthers,
  onCloseAll,
  onPinTab,
  isPinned,
}: {
  x: number;
  y: number;
  onClose: () => void;
  onCloseTab: () => void;
  onCloseOthers: () => void;
  onCloseAll: () => void;
  onPinTab: () => void;
  isPinned: boolean;
}) {
  return (
    <>
      <div
        className="fixed inset-0 z-40"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div
        className={cn([
          "fixed z-50 min-w-35 py-1",
          "rounded-xs border border-neutral-200 bg-white shadow-lg",
        ])}
        style={{ left: x, top: y }}
      >
        <ContextMenuItem
          icon={XIcon}
          label="Close"
          onClick={() => {
            onCloseTab();
            onClose();
          }}
        />
        <ContextMenuItem
          icon={XIcon}
          label="Close others"
          onClick={() => {
            onCloseOthers();
            onClose();
          }}
        />
        <ContextMenuItem
          icon={XIcon}
          label="Close all"
          onClick={() => {
            onCloseAll();
            onClose();
          }}
        />
        <div className="my-1 border-t border-neutral-200" />
        <ContextMenuItem
          icon={isPinned ? PinOffIcon : PinIcon}
          label={isPinned ? "Unpin tab" : "Pin tab"}
          onClick={() => {
            onPinTab();
            onClose();
          }}
        />
      </div>
    </>
  );
}

function FileList({
  filteredItems,
  onFileClick,
}: {
  filteredItems: ContentItem[];
  onFileClick: (item: ContentItem) => void;
}) {
  if (filteredItems.length === 0) {
    return <EmptyState icon={FileTextIcon} message="No files found" />;
  }

  return (
    <div className="flex-1 overflow-y-auto p-4">
      <div className="flex flex-col gap-1">
        {filteredItems.map((item) => (
          <FileItem
            key={item.path}
            item={item}
            onClick={() => onFileClick(item)}
          />
        ))}
      </div>
    </div>
  );
}

function AuthorSelect({
  value,
  onChange,
  withBorder,
}: {
  value: string[];
  onChange: (value: string[]) => void;
  withBorder?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const selectedAuthors = AUTHORS.filter((a) => value.includes(a.name));

  const toggleAuthor = (name: string) => {
    if (value.includes(name)) {
      onChange(value.filter((v) => v !== name));
    } else {
      onChange([...value, name]);
    }
  };

  return (
    <div ref={ref} className="relative flex-1">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn([
          "flex w-full cursor-pointer items-center gap-2 text-left text-neutral-900",
          withBorder &&
            "rounded border border-neutral-200 px-2 py-1.5 focus:border-neutral-400",
        ])}
      >
        {selectedAuthors.length > 0 ? (
          <div className="flex flex-wrap items-center gap-1">
            {selectedAuthors.map((a) => (
              <span
                key={a.name}
                className="inline-flex items-center gap-1 text-sm"
              >
                <img
                  src={a.avatar}
                  alt={a.name}
                  className="size-5 rounded-full object-cover"
                />
                {a.name}
                {selectedAuthors.length > 1 && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onChange(value.filter((v) => v !== a.name));
                    }}
                    className="text-neutral-400 hover:text-neutral-600"
                  >
                    ×
                  </button>
                )}
              </span>
            ))}
          </div>
        ) : (
          <span className="text-neutral-400">Select authors</span>
        )}
        <ChevronDownIcon
          className={cn([
            "ml-auto size-3 text-neutral-400 transition-transform",
            isOpen && "rotate-180",
          ])}
        />
      </button>
      {isOpen && (
        <div className="absolute top-full right-0 left-0 z-50 mt-1 rounded-xs border border-neutral-200 bg-white shadow-lg">
          {AUTHORS.map((author) => (
            <button
              key={author.name}
              type="button"
              onClick={() => toggleAuthor(author.name)}
              className={cn([
                "flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left text-sm",
                "transition-colors hover:bg-neutral-100",
                value.includes(author.name) && "bg-neutral-50",
              ])}
            >
              <img
                src={author.avatar}
                alt={author.name}
                className="size-5 rounded-full object-cover"
              />
              {author.name}
              {value.includes(author.name) && (
                <span className="ml-auto text-neutral-500">✓</span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

const CATEGORIES = [
  "Product",
  "Comparisons",
  "Engineering",
  "Founders' notes",
  "Guides",
  "Char Weekly",
];

function CategorySelect({
  value,
  onChange,
  withBorder,
}: {
  value: string;
  onChange: (value: string) => void;
  withBorder?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <div ref={ref} className="relative flex-1">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={cn([
          "flex w-full cursor-pointer items-center gap-2 text-left text-neutral-900",
          withBorder &&
            "rounded border border-neutral-200 px-2 py-1.5 focus:border-neutral-400",
        ])}
      >
        {value ? (
          <span>{value}</span>
        ) : (
          <span className="text-neutral-400">Select category</span>
        )}
        <ChevronDownIcon
          className={cn([
            "ml-auto size-3 text-neutral-400 transition-transform",
            isOpen && "rotate-180",
          ])}
        />
      </button>
      {isOpen && (
        <div className="absolute top-full right-0 left-0 z-50 mt-1 rounded-xs border border-neutral-200 bg-white shadow-lg">
          {CATEGORIES.map((category) => (
            <button
              key={category}
              type="button"
              onClick={() => {
                onChange(category);
                setIsOpen(false);
              }}
              className={cn([
                "w-full cursor-pointer px-3 py-2 text-left text-sm",
                "transition-colors hover:bg-neutral-100",
                value === category && "bg-neutral-50",
              ])}
            >
              {category}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function MetadataRow({
  label,
  required,
  children,
  noBorder,
}: {
  label: string;
  required?: boolean;
  children: React.ReactNode;
  noBorder?: boolean;
}) {
  return (
    <div className={cn(["flex", !noBorder && "border-b border-neutral-200"])}>
      <span className="relative w-24 shrink-0 px-4 py-2 text-neutral-500">
        {required && <span className="absolute left-1 text-red-400">*</span>}
        {label}
      </span>
      {children}
    </div>
  );
}

interface MetadataHandlers {
  metaTitle: string;
  onMetaTitleChange: (value: string) => void;
  displayTitle: string;
  onDisplayTitleChange: (value: string) => void;
  metaDescription: string;
  onMetaDescriptionChange: (value: string) => void;
  author: string[];
  onAuthorChange: (value: string[]) => void;
  date: string;
  onDateChange: (value: string) => void;
  coverImage: string;
  onCoverImageChange: (value: string) => void;
  featured: boolean;
  onFeaturedChange: (value: boolean) => void;
  category: string;
  onCategoryChange: (value: string) => void;
}

function MetadataPanel({
  isExpanded,
  onToggleExpanded,
  filePath,
  handlers,
}: {
  isExpanded: boolean;
  onToggleExpanded: () => void;
  filePath: string;
  handlers: MetadataHandlers;
}) {
  const [isTitleExpanded, setIsTitleExpanded] = useState(false);

  return (
    <div
      key={filePath}
      className={cn([
        "relative shrink-0",
        isExpanded && "border-b border-neutral-200",
      ])}
    >
      <div
        className={cn([
          "overflow-hidden text-sm transition-all duration-200",
          isExpanded ? "max-h-125" : "max-h-0",
        ])}
      >
        <div className="flex border-b border-neutral-200">
          <button
            onClick={() => setIsTitleExpanded(!isTitleExpanded)}
            className="relative flex w-24 shrink-0 items-center justify-between px-4 py-2 text-neutral-500 hover:text-neutral-700"
          >
            <span className="absolute left-1 text-red-400">*</span>
            Title
            <ChevronRightIcon
              className={cn([
                "size-3 transition-transform",
                isTitleExpanded && "rotate-90",
              ])}
            />
          </button>
          <input
            type="text"
            value={handlers.metaTitle}
            onChange={(e) => handlers.onMetaTitleChange(e.target.value)}
            placeholder="SEO meta title"
            className="flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
          />
        </div>
        {isTitleExpanded && (
          <div className="flex border-b border-neutral-200 bg-neutral-50">
            <span className="relative flex w-24 shrink-0 items-center gap-1 px-4 py-2 text-neutral-400">
              <span className="text-neutral-300">└</span>
              Display
            </span>
            <input
              type="text"
              value={handlers.displayTitle}
              onChange={(e) => handlers.onDisplayTitleChange(e.target.value)}
              placeholder={handlers.metaTitle || "Display title (optional)"}
              className="flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
            />
          </div>
        )}
        <MetadataRow label="Author" required>
          <div className="flex-1 px-2 py-2">
            <AuthorSelect
              value={handlers.author}
              onChange={handlers.onAuthorChange}
            />
          </div>
        </MetadataRow>
        <MetadataRow label="Date" required>
          <input
            type="date"
            value={handlers.date}
            onChange={(e) => handlers.onDateChange(e.target.value)}
            className="-ml-1 flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden"
          />
        </MetadataRow>
        <MetadataRow label="Description" required>
          <textarea
            ref={(el) => {
              if (el) {
                el.style.height = "auto";
                el.style.height = `${el.scrollHeight}px`;
              }
            }}
            value={handlers.metaDescription}
            onChange={(e) => handlers.onMetaDescriptionChange(e.target.value)}
            placeholder="Meta description for SEO"
            rows={1}
            onInput={(e) => {
              const target = e.target as HTMLTextAreaElement;
              target.style.height = "auto";
              target.style.height = `${target.scrollHeight}px`;
            }}
            className="flex-1 resize-none bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
          />
        </MetadataRow>
        <MetadataRow label="Category">
          <select
            value={handlers.category}
            onChange={(e) => handlers.onCategoryChange(e.target.value)}
            className="flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden"
          >
            <option value="">Select category</option>
            <option value="Case Study">Case Study</option>
            <option value="Char Weekly">Char Weekly</option>
            <option value="Productivity Hack">Productivity Hack</option>
            <option value="Engineering">Engineering</option>
          </select>
        </MetadataRow>
        <MetadataRow label="Cover">
          <div className="flex flex-1 items-center gap-2 px-2 py-2">
            <input
              type="text"
              value={handlers.coverImage}
              onChange={(e) => handlers.onCoverImageChange(e.target.value)}
              placeholder="/api/images/blog/slug/cover.png"
              className="flex-1 bg-transparent text-neutral-900 outline-hidden placeholder:text-neutral-300"
            />
          </div>
        </MetadataRow>
        <MetadataRow label="Featured" noBorder>
          <div className="flex flex-1 items-center px-2 py-2">
            <input
              type="checkbox"
              checked={handlers.featured}
              onChange={(e) => handlers.onFeaturedChange(e.target.checked)}
              className="rounded"
            />
          </div>
        </MetadataRow>
      </div>
      <button
        onClick={onToggleExpanded}
        className={cn([
          "absolute top-full left-1/2 z-10 -translate-x-1/2",
          "flex items-center justify-center",
          "h-4 w-10 rounded-b-md border border-t-0 border-neutral-200 bg-white",
          "text-neutral-400 hover:text-neutral-600",
          "cursor-pointer transition-colors",
        ])}
      >
        <ChevronDownIcon
          className={cn([
            "size-3 transition-transform duration-200",
            isExpanded && "rotate-180",
          ])}
        />
      </button>
    </div>
  );
}

interface CommitInfo {
  sha: string;
  message: string;
  author: string;
  date: string;
  url: string;
}

function GitHistory({ filePath }: { filePath: string }) {
  const [isExpanded, setIsExpanded] = useState(false);

  const {
    data: commits = [],
    isLoading,
    refetch,
  } = useQuery<CommitInfo[]>({
    queryKey: ["gitHistory", filePath],
    queryFn: async () => {
      if (!filePath) return [];
      const response = await fetch(
        `/api/admin/content/history?path=${encodeURIComponent(filePath)}`,
      );
      if (!response.ok) {
        throw new Error("Failed to fetch history");
      }
      const data = await response.json();
      return data.commits || [];
    },
    enabled: isExpanded && !!filePath,
  });

  return (
    <div className="border-t border-neutral-200">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="flex w-full items-center justify-between px-4 py-3 text-sm text-neutral-600 hover:bg-neutral-50"
      >
        <span className="flex items-center gap-2">
          <GithubIcon className="size-4" />
          Git History
        </span>
        <ChevronDownIcon
          className={cn([
            "size-4 transition-transform",
            isExpanded && "rotate-180",
          ])}
        />
      </button>
      {isExpanded && (
        <div className="flex flex-col gap-2 px-4 pb-4">
          {isLoading ? (
            <div className="flex items-center gap-2 text-xs text-neutral-400">
              <Spinner size={12} />
              Loading...
            </div>
          ) : commits.length === 0 ? (
            <p className="text-xs text-neutral-400">No commit history</p>
          ) : (
            commits.map((commit) => (
              <a
                key={commit.sha}
                href={commit.url}
                target="_blank"
                rel="noopener noreferrer"
                className="block rounded border border-neutral-100 p-2 hover:bg-neutral-50"
              >
                <div className="flex items-center gap-2 text-xs">
                  <code className="rounded bg-neutral-100 px-1 text-neutral-500">
                    {commit.sha}
                  </code>
                  <span className="text-neutral-400">
                    {new Date(commit.date).toLocaleDateString()}
                  </span>
                </div>
                <p className="mt-1 truncate text-xs text-neutral-700">
                  {commit.message}
                </p>
              </a>
            ))
          )}
          {commits.length > 0 && (
            <button
              onClick={() => refetch()}
              className="flex items-center gap-1 text-xs text-neutral-500 hover:text-neutral-700"
            >
              <RefreshCwIcon className="size-3" />
              Refresh
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function MetadataSidePanel({
  filePath,
  handlers,
}: {
  filePath: string;
  handlers: MetadataHandlers;
}) {
  const [isCoverImageSelectorOpen, setIsCoverImageSelectorOpen] =
    useState(false);

  const [isTitleExpanded, setIsTitleExpanded] = useState(false);

  return (
    <div className="text-sm" key={filePath}>
      <div className="flex border-b border-neutral-200">
        <button
          onClick={() => setIsTitleExpanded(!isTitleExpanded)}
          className="relative flex w-24 shrink-0 items-center justify-between px-4 py-2 text-neutral-500 hover:text-neutral-700"
        >
          <span className="absolute left-1 text-red-400">*</span>
          Title
          <ChevronRightIcon
            className={cn([
              "size-3 transition-transform",
              isTitleExpanded && "rotate-90",
            ])}
          />
        </button>
        <input
          type="text"
          value={handlers.metaTitle}
          onChange={(e) => handlers.onMetaTitleChange(e.target.value)}
          placeholder="SEO meta title"
          className="min-w-0 flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
        />
      </div>
      {isTitleExpanded && (
        <div className="flex border-b border-neutral-200 bg-neutral-50">
          <span className="relative flex w-24 shrink-0 items-center gap-1 px-4 py-2 text-neutral-400">
            <span className="text-neutral-300">└</span>
            Display
          </span>
          <input
            type="text"
            value={handlers.displayTitle}
            onChange={(e) => handlers.onDisplayTitleChange(e.target.value)}
            placeholder={handlers.metaTitle || "Display title (optional)"}
            className="min-w-0 flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
          />
        </div>
      )}
      <MetadataRow label="Author" required>
        <div className="min-w-0 flex-1 px-2 py-2">
          <AuthorSelect
            value={handlers.author}
            onChange={handlers.onAuthorChange}
          />
        </div>
      </MetadataRow>
      <MetadataRow label="Date" required>
        <input
          type="date"
          value={handlers.date}
          onChange={(e) => handlers.onDateChange(e.target.value)}
          className="-ml-1 min-w-0 flex-1 bg-transparent px-2 py-2 text-neutral-900 outline-hidden"
        />
      </MetadataRow>
      <MetadataRow label="Description" required>
        <textarea
          ref={(el) => {
            if (el) {
              el.style.height = "auto";
              el.style.height = `${el.scrollHeight}px`;
            }
          }}
          value={handlers.metaDescription}
          onChange={(e) => handlers.onMetaDescriptionChange(e.target.value)}
          placeholder="Meta description for SEO"
          rows={1}
          onInput={(e) => {
            const target = e.target as HTMLTextAreaElement;
            target.style.height = "auto";
            target.style.height = `${target.scrollHeight}px`;
          }}
          className="min-w-0 flex-1 resize-none bg-transparent px-2 py-2 text-neutral-900 outline-hidden placeholder:text-neutral-300"
        />
      </MetadataRow>
      <MetadataRow label="Category">
        <div className="min-w-0 flex-1 px-2 py-2">
          <CategorySelect
            value={handlers.category}
            onChange={handlers.onCategoryChange}
          />
        </div>
      </MetadataRow>
      <MetadataRow label="Cover">
        <button
          type="button"
          onClick={() => setIsCoverImageSelectorOpen(true)}
          className="flex min-w-0 flex-1 cursor-pointer items-center gap-2 px-2 py-2 text-left transition-colors hover:bg-neutral-50"
        >
          {handlers.coverImage ? (
            <span className="flex-1 truncate text-neutral-900">
              {handlers.coverImage}
            </span>
          ) : (
            <span className="flex-1 text-neutral-300">Select cover image</span>
          )}
          <ImageIcon className="size-4 shrink-0 text-neutral-400" />
        </button>
      </MetadataRow>
      <MetadataRow label="Featured" noBorder>
        <div className="flex flex-1 items-center px-2 py-2">
          <input
            type="checkbox"
            checked={handlers.featured}
            onChange={(e) => handlers.onFeaturedChange(e.target.checked)}
            className="rounded"
          />
        </div>
      </MetadataRow>

      <GitHistory filePath={filePath} />

      <MediaSelectorModal
        open={isCoverImageSelectorOpen}
        onOpenChange={setIsCoverImageSelectorOpen}
        onSelect={(url) => {
          handlers.onCoverImageChange(url);
          setIsCoverImageSelectorOpen(false);
        }}
      />
    </div>
  );
}

interface BranchFileResponse {
  success: boolean;
  content: string;
  frontmatter: {
    meta_title?: string;
    display_title?: string;
    meta_description?: string;
    author?: string | string[];
    date?: string;
    coverImage?: string;
    featured?: boolean;
    category?: string;
  };
  sha: string;
}

const FileEditor = React.forwardRef<
  FileEditorHandle,
  {
    filePath: string;
    branch?: string;
    isPreviewMode: boolean;
    onDataChange: (data: EditorData) => void;
    onSave: (options?: { isAutoSave?: boolean }) => void;
    isSaving: boolean;
  }
>(function FileEditor(
  { filePath, branch, isPreviewMode, onDataChange, onSave },
  _ref,
) {
  const {
    data: branchFileData,
    isLoading: isBranchLoading,
    error: branchError,
  } = useQuery({
    queryKey: ["branchFile", filePath, branch],
    queryFn: async () => {
      const params = new URLSearchParams({
        path: `apps/web/content/${filePath}`,
        branch: branch!,
      });
      const response = await fetch(
        `/api/admin/content/get-branch-file?${params}`,
      );
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Failed to fetch file from branch");
      }
      return response.json() as Promise<BranchFileResponse>;
    },
    enabled: !!branch,
    staleTime: 30000,
  });

  const { data: pendingPRData } = useQuery({
    queryKey: ["pendingPR", filePath],
    queryFn: async () => {
      const params = new URLSearchParams({ path: filePath });
      const response = await fetch(`/api/admin/content/pending-pr?${params}`);
      if (!response.ok) {
        return { hasPendingPR: false };
      }
      return response.json() as Promise<{
        hasPendingPR: boolean;
        prNumber?: number;
        prUrl?: string;
        branchName?: string;
      }>;
    },
    enabled: !branch && filePath.startsWith("articles/"),
    staleTime: 60000,
  });

  const { data: pendingPRFileData, isLoading: isPendingPRLoading } = useQuery({
    queryKey: ["pendingPRFile", filePath, pendingPRData?.branchName],
    queryFn: async () => {
      const params = new URLSearchParams({
        path: `apps/web/content/${filePath}`,
        branch: pendingPRData!.branchName!,
      });
      const response = await fetch(
        `/api/admin/content/get-branch-file?${params}`,
      );
      if (!response.ok) {
        throw new Error("Failed to fetch file from PR branch");
      }
      return response.json() as Promise<BranchFileResponse>;
    },
    enabled: !!pendingPRData?.hasPendingPR && !!pendingPRData?.branchName,
    staleTime: 30000,
  });

  const publishedFileContent = useMemo(
    () => getFileContent(filePath),
    [filePath],
  );

  const fileContent: FileContent | undefined = useMemo(() => {
    if (branch && branchFileData) {
      return {
        content: branchFileData.content,
        mdx: "",
        collection: "articles",
        slug: filePath.replace(/\.mdx$/, "").replace(/^articles\//, ""),
        meta_title: branchFileData.frontmatter.meta_title,
        display_title: branchFileData.frontmatter.display_title,
        meta_description: branchFileData.frontmatter.meta_description,
        author: Array.isArray(branchFileData.frontmatter.author)
          ? branchFileData.frontmatter.author
          : branchFileData.frontmatter.author
            ? [branchFileData.frontmatter.author]
            : undefined,
        date: branchFileData.frontmatter.date,
        coverImage: branchFileData.frontmatter.coverImage,
        featured: branchFileData.frontmatter.featured,
        category: branchFileData.frontmatter.category,
      };
    }
    if (pendingPRData?.hasPendingPR && pendingPRFileData) {
      return {
        content: pendingPRFileData.content,
        mdx: "",
        collection: "articles",
        slug: filePath.replace(/\.mdx$/, "").replace(/^articles\//, ""),
        meta_title: pendingPRFileData.frontmatter.meta_title,
        display_title: pendingPRFileData.frontmatter.display_title,
        meta_description: pendingPRFileData.frontmatter.meta_description,
        author: Array.isArray(pendingPRFileData.frontmatter.author)
          ? pendingPRFileData.frontmatter.author
          : pendingPRFileData.frontmatter.author
            ? [pendingPRFileData.frontmatter.author]
            : undefined,
        date: pendingPRFileData.frontmatter.date,
        coverImage: pendingPRFileData.frontmatter.coverImage,
        featured: pendingPRFileData.frontmatter.featured,
        category: pendingPRFileData.frontmatter.category,
      };
    }
    return publishedFileContent;
  }, [
    branch,
    branchFileData,
    pendingPRData,
    pendingPRFileData,
    publishedFileContent,
    filePath,
  ]);

  const [content, setContent] = useState(fileContent?.content || "");
  const [metaTitle, setMetaTitle] = useState(fileContent?.meta_title || "");
  const [displayTitle, setDisplayTitle] = useState(
    fileContent?.display_title || "",
  );
  const [metaDescription, setMetaDescription] = useState(
    fileContent?.meta_description || "",
  );
  const [author, setAuthor] = useState<string[]>(fileContent?.author || []);
  const [date, setDate] = useState(fileContent?.date || "");
  const [coverImage, setCoverImage] = useState(fileContent?.coverImage || "");
  const [featured, setFeatured] = useState(fileContent?.featured || false);
  const [category, setCategory] = useState(fileContent?.category || "");

  const [isMetadataExpanded, setIsMetadataExpanded] = useState(true);
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [isMediaSelectorOpen, setIsMediaSelectorOpen] = useState(false);
  const [autoSaveCountdown, setAutoSaveCountdown] = useState<number | null>(
    null,
  );
  const autoSaveIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const onSaveRef = useRef(onSave);

  const handleImageUpload = useCallback(
    async (file: File): Promise<{ url: string; attachmentId: string }> => {
      const result = await uploadBlogImageFile({ file });
      return { url: result.publicUrl, attachmentId: "" };
    },
    [],
  );

  const editor = useBlogEditor({
    content: fileContent?.content || "",
    onUpdate: (markdown) => {
      setContent(markdown);
      setHasUnsavedChanges(true);
    },
    onImageUpload: handleImageUpload,
  });

  const slug = filePath.replace(/\.mdx$/, "").replace(/^articles\//, "");

  const { mutate: importFromDocs, isPending: isImporting } = useMutation({
    mutationFn: async (params: {
      url: string;
      title?: string;
      author?: string | string[];
      description?: string;
      coverImage?: string;
      slug?: string;
    }) => {
      const response = await fetch("/api/admin/import/google-docs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      });
      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || "Import failed");
      }
      return response.json() as Promise<ImportResult>;
    },
    onSuccess: (data) => {
      if (data.md) {
        editor?.commands.setContent(data.md, { contentType: "markdown" });
      }
      if (data.frontmatter) {
        if (data.frontmatter.meta_title)
          setMetaTitle(data.frontmatter.meta_title);
        if (data.frontmatter.display_title)
          setDisplayTitle(data.frontmatter.display_title);
        if (data.frontmatter.meta_description)
          setMetaDescription(data.frontmatter.meta_description);
        if (data.frontmatter.author) setAuthor(data.frontmatter.author);
        if (data.frontmatter.date) setDate(data.frontmatter.date);
        if (data.frontmatter.coverImage)
          setCoverImage(data.frontmatter.coverImage);
      }
      setHasUnsavedChanges(true);
    },
  });

  const handleGoogleDocsImport = useCallback(
    (url: string) => {
      importFromDocs({
        url,
        slug,
        title: metaTitle,
        author,
        description: metaDescription,
        coverImage,
      });
    },
    [importFromDocs, slug, metaTitle, author, metaDescription, coverImage],
  );

  const handleMediaLibrarySelect = useCallback(
    (publicUrl: string) => {
      if (editor) {
        editor
          .chain()
          .focus()
          .insertContent({
            type: "image",
            attrs: { src: publicUrl },
          })
          .run();
        setContent(getEditorMarkdown(editor, content));
        setHasUnsavedChanges(true);
      }
      setIsMediaSelectorOpen(false);
    },
    [content, editor],
  );

  const getMetadata = useCallback(
    (): ArticleMetadata => ({
      meta_title: metaTitle,
      display_title: displayTitle,
      meta_description: metaDescription,
      author,
      date,
      coverImage,
      featured,
      category,
    }),
    [
      metaTitle,
      displayTitle,
      metaDescription,
      author,
      date,
      coverImage,
      featured,
      category,
    ],
  );

  const getCurrentData = useCallback((): EditorData | null => {
    return {
      content: getEditorMarkdown(editor, content),
      metadata: getMetadata(),
      hasUnsavedChanges,
      autoSaveCountdown,
    };
  }, [autoSaveCountdown, content, editor, getMetadata, hasUnsavedChanges]);

  React.useImperativeHandle(
    _ref,
    () => ({
      getData: getCurrentData,
    }),
    [getCurrentData],
  );

  useEffect(() => {
    const newContent = fileContent?.content || "";
    setContent(newContent);
    setMetaTitle(fileContent?.meta_title || "");
    setDisplayTitle(fileContent?.display_title || "");
    setMetaDescription(fileContent?.meta_description || "");
    setAuthor(fileContent?.author || []);
    setDate(fileContent?.date || "");
    setCoverImage(fileContent?.coverImage || "");
    setFeatured(fileContent?.featured || false);
    setCategory(fileContent?.category || "");
    setHasUnsavedChanges(false);
    if (editor) {
      editor.commands.setContent(newContent, {
        contentType: "markdown",
        emitUpdate: false,
      });
    }
  }, [filePath, fileContent, pendingPRData?.hasPendingPR, editor]);

  useEffect(() => {
    onDataChange({
      content,
      metadata: getMetadata(),
      hasUnsavedChanges,
      autoSaveCountdown,
    });
  }, [
    content,
    metaTitle,
    displayTitle,
    metaDescription,
    author,
    date,
    coverImage,
    featured,
    category,
    onDataChange,
    getMetadata,
    hasUnsavedChanges,
    autoSaveCountdown,
  ]);

  useEffect(() => {
    onSaveRef.current = onSave;
  }, [onSave]);

  useEffect(() => {
    if (!hasUnsavedChanges) {
      setAutoSaveCountdown(null);
      if (autoSaveIntervalRef.current) {
        clearInterval(autoSaveIntervalRef.current);
        autoSaveIntervalRef.current = null;
      }
      return;
    }

    setAutoSaveCountdown(60);

    autoSaveIntervalRef.current = setInterval(() => {
      setAutoSaveCountdown((prev) => {
        if (prev === null || prev <= 1) {
          onSaveRef.current({ isAutoSave: true });
          setHasUnsavedChanges(false);
          return null;
        }
        return prev - 1;
      });
    }, 1000);

    return () => {
      if (autoSaveIntervalRef.current) {
        clearInterval(autoSaveIntervalRef.current);
        autoSaveIntervalRef.current = null;
      }
    };
  }, [hasUnsavedChanges]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        onSaveRef.current();
        setHasUnsavedChanges(false);
        setAutoSaveCountdown(null);
        if (autoSaveIntervalRef.current) {
          clearInterval(autoSaveIntervalRef.current);
          autoSaveIntervalRef.current = null;
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const handleBeforeUnload = (e: BeforeUnloadEvent) => {
      if (hasUnsavedChanges) {
        e.preventDefault();
        return "";
      }
    };
    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [hasUnsavedChanges]);

  if (branch && isBranchLoading) {
    return (
      <div className="flex flex-1 items-center justify-center text-neutral-500">
        <div className="text-center">
          <Spinner size={32} />
          <p className="mt-3 text-sm">Loading draft...</p>
        </div>
      </div>
    );
  }

  if (isPendingPRLoading && pendingPRData?.hasPendingPR) {
    return (
      <div className="flex flex-1 items-center justify-center text-neutral-500">
        <div className="text-center">
          <Spinner size={32} />
          <p className="mt-3 text-sm">Loading from pending PR...</p>
        </div>
      </div>
    );
  }

  if (branch && branchError) {
    return (
      <div className="flex flex-1 items-center justify-center text-neutral-500">
        <div className="text-center">
          <FileWarningIcon className="mb-3 size-10" />
          <p className="text-sm">Failed to load draft</p>
          <p className="mt-1 text-xs text-neutral-400">
            {branchError instanceof Error
              ? branchError.message
              : "Unknown error"}
          </p>
        </div>
      </div>
    );
  }

  if (!fileContent) {
    return (
      <div className="flex flex-1 items-center justify-center text-neutral-500">
        <div className="text-center">
          <FileWarningIcon className="mb-3 size-10" />
          <p className="text-sm">File not found</p>
        </div>
      </div>
    );
  }

  const selectedAuthors = AUTHORS.filter((a) => author.includes(a.name));

  const dirty = <T,>(setter: React.Dispatch<React.SetStateAction<T>>) =>
    ((value: React.SetStateAction<T>) => {
      setter(value);
      setHasUnsavedChanges(true);
    }) as React.Dispatch<React.SetStateAction<T>>;

  const metadataHandlers: MetadataHandlers = {
    metaTitle,
    onMetaTitleChange: dirty(setMetaTitle),
    displayTitle,
    onDisplayTitleChange: dirty(setDisplayTitle),
    metaDescription,
    onMetaDescriptionChange: dirty(setMetaDescription),
    author,
    onAuthorChange: dirty(setAuthor),
    date,
    onDateChange: dirty(setDate),
    coverImage,
    onCoverImageChange: dirty(setCoverImage),
    featured,
    onFeaturedChange: dirty(setFeatured),
    category,
    onCategoryChange: dirty(setCategory),
  };

  const renderPreview = () => (
    <div className="h-full overflow-y-auto bg-white">
      <header className="mx-auto max-w-3xl px-6 py-12 text-center">
        <h1 className="mb-6 font-serif text-3xl text-stone-600">
          {fileContent.display_title || fileContent.meta_title || "Untitled"}
        </h1>
        {author.length > 0 && (
          <div className="mb-2 flex items-center justify-center gap-3">
            {selectedAuthors.map((a) => (
              <div key={a.name} className="flex items-center gap-2">
                <img
                  src={a.avatar}
                  alt={a.name}
                  className="h-8 w-8 rounded-full object-cover"
                />
                <p className="text-base text-neutral-600">{a.name}</p>
              </div>
            ))}
          </div>
        )}
        {fileContent.date && (
          <time className="font-mono text-xs text-neutral-500">
            {new Date(fileContent.date).toLocaleDateString("en-US", {
              year: "numeric",
              month: "long",
              day: "numeric",
            })}
          </time>
        )}
      </header>
      <div className="mx-auto max-w-3xl px-6 pb-8">
        <article className="prose prose-stone prose-headings:font-serif prose-headings:font-semibold prose-h1:text-3xl prose-h1:mt-12 prose-h1:mb-6 prose-h2:text-2xl prose-h2:mt-10 prose-h2:mb-5 prose-h3:text-xl prose-h3:mt-8 prose-h3:mb-4 prose-h4:text-lg prose-h4:mt-6 prose-h4:mb-3 prose-a:text-stone-600 prose-a:underline prose-a:decoration-dotted hover:prose-a:text-stone-800 prose-headings:no-underline prose-headings:decoration-transparent prose-code:bg-stone-50 prose-code:border prose-code:border-neutral-200 prose-code:rounded prose-code:px-1.5 prose-code:py-0.5 prose-code:text-sm prose-code:font-mono prose-code:text-stone-700 prose-pre:bg-stone-50 prose-pre:border prose-pre:border-neutral-200 prose-pre:rounded-xs prose-pre:prose-code:bg-transparent prose-pre:prose-code:border-0 prose-pre:prose-code:p-0 prose-img:rounded-xs prose-img:border prose-img:border-neutral-200 prose-img:my-8 max-w-none">
          {fileContent.mdx ? (
            <MDXContent
              code={fileContent.mdx}
              components={defaultMDXComponents}
            />
          ) : (
            <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
          )}
        </article>
      </div>
    </div>
  );

  const pendingPRBanner = pendingPRData?.hasPendingPR ? (
    <div className="flex items-center justify-between border-b border-amber-200 bg-amber-50 px-4 py-2">
      <div className="flex items-center gap-2 text-sm text-amber-800">
        <AlertTriangleIcon className="size-4" />
        <span>
          This article has a pending edit PR. Your changes will be added to{" "}
          <a
            href={pendingPRData.prUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="font-medium underline hover:text-amber-900"
          >
            PR #{pendingPRData.prNumber}
          </a>
        </span>
      </div>
      <a
        href={pendingPRData.prUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="text-xs font-medium text-amber-700 hover:text-amber-900"
      >
        View PR →
      </a>
    </div>
  ) : null;

  if (isPreviewMode) {
    return (
      <>
        {pendingPRBanner}
        <ResizablePanelGroup direction="horizontal" className="min-h-0 flex-1">
          <ResizablePanel defaultSize={50} minSize={30}>
            <div className="flex h-full flex-col">
              <MetadataPanel
                isExpanded={isMetadataExpanded}
                onToggleExpanded={() =>
                  setIsMetadataExpanded(!isMetadataExpanded)
                }
                filePath={filePath}
                handlers={metadataHandlers}
              />
              <BlogEditor
                editor={editor}
                onGoogleDocsImport={handleGoogleDocsImport}
                isImporting={isImporting}
                onAddImageFromLibrary={() => setIsMediaSelectorOpen(true)}
                showToolbar={false}
              />
            </div>
          </ResizablePanel>
          <ResizableHandle className="w-px bg-neutral-200" />
          <ResizablePanel defaultSize={50} minSize={30}>
            {renderPreview()}
          </ResizablePanel>
        </ResizablePanelGroup>

        <MediaSelectorModal
          open={isMediaSelectorOpen}
          onOpenChange={setIsMediaSelectorOpen}
          onSelect={handleMediaLibrarySelect}
        />
      </>
    );
  }

  return (
    <>
      {pendingPRBanner}
      <ResizablePanelGroup direction="horizontal" className="min-h-0 flex-1">
        <ResizablePanel defaultSize={70} minSize={50}>
          <BlogEditor
            editor={editor}
            onGoogleDocsImport={handleGoogleDocsImport}
            isImporting={isImporting}
            onAddImageFromLibrary={() => setIsMediaSelectorOpen(true)}
          />
        </ResizablePanel>
        <ResizableHandle className="w-px bg-neutral-200" />
        <ResizablePanel defaultSize={30} minSize={20}>
          <div className="h-full overflow-y-auto">
            <MetadataSidePanel
              filePath={filePath}
              handlers={metadataHandlers}
            />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>

      <MediaSelectorModal
        open={isMediaSelectorOpen}
        onOpenChange={setIsMediaSelectorOpen}
        onSelect={handleMediaLibrarySelect}
      />
    </>
  );
});

function EmptyState({
  icon: Icon,
  message,
}: {
  icon: LucideIcon;
  message: string;
}) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center text-neutral-500">
      <Icon className="mb-3 size-10" />
      <p className="text-sm">{message}</p>
    </div>
  );
}

function FileItem({
  item,
  onClick,
}: {
  item: ContentItem;
  onClick: () => void;
}) {
  return (
    <div
      className={cn([
        "flex cursor-pointer items-center justify-between rounded px-3 py-2",
        "transition-colors hover:bg-neutral-50",
        "border border-transparent hover:border-neutral-200",
      ])}
      onClick={onClick}
    >
      <div className="flex items-center gap-2">
        <FileTextIcon className="size-4 text-neutral-400" />
        <span className="text-sm text-neutral-700">
          {item.name.replace(/\.mdx$/, "")}
        </span>
        <span className="rounded bg-neutral-100 px-1.5 py-0.5 text-xs text-neutral-400">
          {getFileExtension(item.name).toUpperCase()}
        </span>
      </div>
      <a
        href={`https://github.com/fastrepl/char/blob/main/apps/web/content/${item.path}`}
        target="_blank"
        rel="noopener noreferrer"
        className="text-xs text-neutral-500 hover:text-neutral-700"
        onClick={(e) => e.stopPropagation()}
      >
        <GithubIcon className="size-4" />
      </a>
    </div>
  );
}

interface ImportResult {
  success: boolean;
  md?: string;
  frontmatter?: {
    meta_title: string;
    display_title: string;
    meta_description: string;
    author: string[];
    coverImage: string;
    featured: boolean;
    date: string;
  };
  error?: string;
}
