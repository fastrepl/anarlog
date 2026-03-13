import { useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  AlertCircleIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  DownloadIcon,
  FileIcon,
  FolderIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  HomeIcon,
  MoreVerticalIcon,
  MoveIcon,
  PencilIcon,
  PinIcon,
  PinOffIcon,
  PlusIcon,
  RefreshCwIcon,
  SearchIcon,
  Trash2Icon,
  UploadIcon,
  XIcon,
} from "lucide-react";
import { Reorder } from "motion/react";
import React, { useCallback, useEffect, useRef, useState } from "react";

import {
  Dialog,
  DialogContent,
  DialogFooter,
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
import { cn } from "@hypr/utils";

import {
  fetchMediaItems,
  type MediaItem,
  useMediaApi,
} from "@/hooks/use-media-api";

interface TreeNode {
  path: string;
  name: string;
  type: "file" | "dir";
  expanded: boolean;
  loaded: boolean;
  children: TreeNode[];
}

interface Tab {
  id: string;
  type: "folder" | "file";
  name: string;
  path: string;
  pinned: boolean;
  active: boolean;
  isHome?: boolean;
}

export const Route = createFileRoute("/admin/media/")({
  component: MediaLibrary,
});

function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function getRelativePath(fullPath: string): string {
  return fullPath;
}

function MediaLibrary() {
  const queryClient = useQueryClient();
  const [searchQuery, setSearchQuery] = useState("");
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [treeNodes, setTreeNodes] = useState<TreeNode[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [selectedItems, setSelectedItems] = useState<Set<string>>(new Set());
  const [dragOver, setDragOver] = useState(false);
  const [draggingItem, setDraggingItem] = useState<MediaItem | null>(null);
  const [dropTargetPath, setDropTargetPath] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [isMounted, setIsMounted] = useState(false);
  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());

  const [isCreatingFolder, setIsCreatingFolder] = useState(false);
  const [showMoveModal, setShowMoveModal] = useState(false);
  const [itemToMove, setItemToMove] = useState<MediaItem | null>(null);

  const [navigationHistory, setNavigationHistory] = useState<
    Array<{ path: string; name: string }>
  >([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const isNavigatingRef = useRef(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  const rootQuery = useQuery({
    queryKey: ["mediaItems", ""],
    queryFn: () => fetchMediaItems(""),
    enabled: isMounted,
  });

  useEffect(() => {
    if (rootQuery.data && !rootLoaded) {
      const children: TreeNode[] = rootQuery.data.map((item) => ({
        path: getRelativePath(item.path),
        name: item.name,
        type: item.type,
        expanded: false,
        loaded: false,
        children: [],
      }));
      setTreeNodes(children);
      setRootLoaded(true);

      // Add permanent Home tab
      setTabs([
        {
          id: "home",
          type: "folder",
          name: "Home",
          path: "",
          pinned: true,
          active: true,
          isHome: true,
        },
      ]);
    }
  }, [rootQuery.data, rootLoaded]);

  const currentTab = tabs.find((t) => t.active);

  const currentPathQuery = useQuery({
    queryKey: ["mediaItems", currentTab?.path || "", currentTab?.type],
    queryFn: async () => {
      if (currentTab?.type === "file") {
        const parentPath = currentTab.path.split("/").slice(0, -1).join("/");
        const items = await fetchMediaItems(parentPath);
        return items.filter((i) => i.path === currentTab.path);
      }
      return fetchMediaItems(currentTab?.path || "");
    },
    enabled: isMounted && currentTab !== undefined,
  });

  const loadFolderContents = async (path: string) => {
    setLoadingPaths((prev) => new Set(prev).add(path));
    try {
      const items = await queryClient.fetchQuery({
        queryKey: ["mediaItems", path],
        queryFn: () => fetchMediaItems(path),
      });
      const children: TreeNode[] = items.map((item) => ({
        path: getRelativePath(item.path),
        name: item.name,
        type: item.type,
        expanded: false,
        loaded: false,
        children: [],
      }));

      if (path === "") {
        setTreeNodes(children);
        setRootLoaded(true);
      } else {
        setTreeNodes((prev) => updateTreeNode(prev, path, children));
      }
    } finally {
      setLoadingPaths((prev) => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    }
  };

  const updateTreeNode = (
    nodes: TreeNode[],
    targetPath: string,
    children: TreeNode[],
  ): TreeNode[] => {
    return nodes.map((node) => {
      if (node.path === targetPath) {
        return { ...node, children, loaded: true };
      }
      if (node.children.length > 0) {
        return {
          ...node,
          children: updateTreeNode(node.children, targetPath, children),
        };
      }
      return node;
    });
  };

  const toggleNodeExpanded = async (path: string) => {
    const node = findNode(treeNodes, path);
    if (!node) return;

    const willExpand = !node.expanded;
    if (willExpand && !node.loaded && node.type === "dir") {
      await loadFolderContents(path);
    }
    setTreeNodes((prev) => toggleExpanded(prev, path));
  };

  const findNode = (nodes: TreeNode[], path: string): TreeNode | null => {
    for (const node of nodes) {
      if (node.path === path) return node;
      if (node.children.length > 0) {
        const found = findNode(node.children, path);
        if (found) return found;
      }
    }
    return null;
  };

  const toggleExpanded = (nodes: TreeNode[], path: string): TreeNode[] => {
    return nodes.map((node) => {
      if (node.path === path) {
        return { ...node, expanded: !node.expanded };
      }
      if (node.children.length > 0) {
        return { ...node, children: toggleExpanded(node.children, path) };
      }
      return node;
    });
  };

  const openTab = useCallback(
    (
      type: "folder" | "file",
      name: string,
      path: string,
      options: { pinned?: boolean; forceNewTab?: boolean } = {},
    ) => {
      const { pinned = false, forceNewTab = false } = options;
      setTabs((prev) => {
        // If opening the home/root folder, just activate the Home tab
        if (type === "folder" && path === "") {
          return prev.map((t) => ({ ...t, active: t.isHome === true }));
        }

        const existingIndex = prev.findIndex(
          (t) => t.type === type && t.path === path,
        );
        if (existingIndex !== -1) {
          return prev.map((t, i) => ({ ...t, active: i === existingIndex }));
        }

        const newTab: Tab = {
          id: `${type}-${path}-${Date.now()}`,
          type,
          name,
          path,
          pinned,
          active: true,
        };

        if (forceNewTab) {
          return [...prev.map((t) => ({ ...t, active: false })), newTab];
        }

        const unpinnedIndex = prev.findIndex((t) => !t.pinned && !t.isHome);
        if (unpinnedIndex !== -1 && prev.length > 0) {
          return prev.map((t, i) =>
            i === unpinnedIndex ? newTab : { ...t, active: false },
          );
        }

        return [...prev.map((t) => ({ ...t, active: false })), newTab];
      });
      setSelectedItems(new Set());
    },
    [],
  );

  const closeTab = useCallback((tabId: string) => {
    setTabs((prev) => {
      const index = prev.findIndex((t) => t.id === tabId);
      if (index === -1) return prev;

      // Don't allow closing the Home tab
      if (prev[index].isHome) return prev;

      const newTabs = prev.filter((t) => t.id !== tabId);
      if (newTabs.length === 0) return [];

      if (prev[index].active) {
        const newActiveIndex = Math.min(index, newTabs.length - 1);
        return newTabs.map((t, i) => ({ ...t, active: i === newActiveIndex }));
      }
      return newTabs;
    });
  }, []);

  const selectTab = useCallback((tabId: string) => {
    setTabs((prev) => prev.map((t) => ({ ...t, active: t.id === tabId })));
    setSelectedItems(new Set());
  }, []);

  const pinTab = useCallback((tabId: string) => {
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, pinned: !t.pinned } : t)),
    );
  }, []);

  const reorderTabs = useCallback((newTabs: Tab[]) => {
    setTabs(newTabs);
  }, []);

  const navigateToFolder = useCallback(
    (path: string, name: string, addToHistory = true) => {
      if (addToHistory && !isNavigatingRef.current) {
        setNavigationHistory((prev) => {
          const newHistory = prev.slice(0, historyIndex + 1);
          newHistory.push({ path, name: name || "Home" });
          if (newHistory.length > 50) {
            newHistory.shift();
          }
          return newHistory;
        });
        setHistoryIndex((prev) => Math.min(prev + 1, 49));
      }
      isNavigatingRef.current = false;
      openTab("folder", name || "Home", path);
    },
    [historyIndex, openTab],
  );

  const handleNavigateBack = useCallback(() => {
    if (historyIndex > 0) {
      isNavigatingRef.current = true;
      const prevEntry = navigationHistory[historyIndex - 1];
      setHistoryIndex(historyIndex - 1);
      openTab("folder", prevEntry.name, prevEntry.path);
    }
  }, [historyIndex, navigationHistory, openTab]);

  const handleNavigateForward = useCallback(() => {
    if (historyIndex < navigationHistory.length - 1) {
      isNavigatingRef.current = true;
      const nextEntry = navigationHistory[historyIndex + 1];
      setHistoryIndex(historyIndex + 1);
      openTab("folder", nextEntry.name, nextEntry.path);
    }
  }, [historyIndex, navigationHistory, openTab]);

  const {
    uploadMutation,
    deleteMutation,
    replaceMutation,
    createFolderMutation,
    moveMutation,
    renameMutation,
  } = useMediaApi({
    currentFolderPath: currentTab?.type === "folder" ? currentTab.path : "",
    onFolderCreated: (parentFolder) => {
      loadFolderContents(parentFolder);
      if (parentFolder === "") {
        setRootLoaded(false);
      }
    },
    onFileMoved: () => {
      setShowMoveModal(false);
      setItemToMove(null);
      loadFolderContents(currentTab?.path || "");
    },
    onSelectionCleared: () => {
      setSelectedItems(new Set());
      if (currentTab?.type === "folder") {
        loadFolderContents(currentTab.path);
      }
    },
  });

  const handleCreateFolder = (name: string) => {
    const parentFolder = currentTab?.type === "folder" ? currentTab.path : "";
    createFolderMutation.mutate({ name, parentFolder });
    setIsCreatingFolder(false);
  };

  const handleMoveFile = (destinationFolder: string) => {
    if (!itemToMove) return;
    const fileName = itemToMove.path.split("/").pop() || "";
    const toPath = destinationFolder
      ? `${destinationFolder}/${fileName}`
      : fileName;
    moveMutation.mutate({ fromPath: itemToMove.path, toPath });
  };

  const handleDragItemStart = (item: MediaItem) => {
    setDraggingItem(item);
  };

  const handleDragItemEnd = () => {
    setDraggingItem(null);
    setDropTargetPath(null);
  };

  const handleDropOnFolder = (targetFolderPath: string) => {
    if (!draggingItem) return;
    if (draggingItem.path === targetFolderPath) return;
    if (draggingItem.path.startsWith(targetFolderPath + "/")) return;

    const fileName = draggingItem.path.split("/").pop() || "";
    const toPath = targetFolderPath
      ? `${targetFolderPath}/${fileName}`
      : fileName;

    if (draggingItem.path !== toPath) {
      moveMutation.mutate({ fromPath: draggingItem.path, toPath });
    }

    setDraggingItem(null);
    setDropTargetPath(null);
  };

  const openMoveModal = (item: MediaItem) => {
    setItemToMove(item);
    setShowMoveModal(true);
  };

  const handleRename = (path: string, newName: string) => {
    renameMutation.mutate({ path, newName });
  };

  const handleUpload = (files: FileList) => {
    uploadMutation.mutate(files);
  };

  const handleDelete = () => {
    if (selectedItems.size === 0) return;
    if (
      !confirm(`Are you sure you want to delete ${selectedItems.size} item(s)?`)
    )
      return;
    deleteMutation.mutate(Array.from(selectedItems));
  };

  const handleDownload = (publicUrl: string, filename: string) => {
    const link = document.createElement("a");
    link.href = publicUrl;
    link.download = filename;
    link.target = "_blank";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const handleDownloadSelected = () => {
    const currentItems = currentPathQuery.data || [];
    selectedItems.forEach((path) => {
      const item = currentItems.find((i) => i.path === path);
      if (item && item.type === "file") {
        handleDownload(item.publicUrl, item.name);
      }
    });
  };

  const handleReplace = (file: File, path: string) => {
    replaceMutation.mutate({ file, path });
  };

  const handleDeleteSingle = (path: string) => {
    if (!confirm(`Are you sure you want to delete this file?`)) return;
    deleteMutation.mutate([path]);
  };

  const toggleSelection = (path: string) => {
    const newSelection = new Set(selectedItems);
    if (newSelection.has(path)) {
      newSelection.delete(path);
    } else {
      newSelection.add(path);
    }
    setSelectedItems(newSelection);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      handleUpload(e.dataTransfer.files);
    }
  };

  const filterTreeNodes = (nodes: TreeNode[], query: string): TreeNode[] => {
    if (!query) return nodes;
    const lowerQuery = query.toLowerCase();

    return nodes
      .map((node) => {
        const matchesName = node.name.toLowerCase().includes(lowerQuery);
        const filteredChildren = filterTreeNodes(node.children, query);

        if (matchesName || filteredChildren.length > 0) {
          return { ...node, children: filteredChildren, expanded: true };
        }
        return null;
      })
      .filter((node): node is TreeNode => node !== null);
  };

  const filteredTreeNodes = filterTreeNodes(treeNodes, searchQuery);

  return (
    <>
      <ResizablePanelGroup
        direction="horizontal"
        className="h-[calc(100vh-64px)]"
      >
        <ResizablePanel defaultSize={20} minSize={15} maxSize={30}>
          <Sidebar
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            loadingPaths={loadingPaths}
            filteredTreeNodes={filteredTreeNodes}
            onOpenFolder={(path, name) => openTab("folder", name, path)}
            onOpenFile={(path, name) => openTab("file", name, path)}
            onToggleNodeExpanded={toggleNodeExpanded}
            uploadPending={uploadMutation.isPending}
            fileInputRef={fileInputRef}
            onUpload={handleUpload}
            isCreatingFolder={isCreatingFolder}
            onCreateFolderClick={() => setIsCreatingFolder(true)}
            onCreateFolder={handleCreateFolder}
            onCancelCreateFolder={() => setIsCreatingFolder(false)}
            createFolderPending={createFolderMutation.isPending}
            currentTab={currentTab}
            onRename={handleRename}
            onMove={(path, name, type) => {
              setItemToMove({
                id: path,
                path,
                name,
                type,
                size: 0,
                mimeType: null,
                publicUrl: "",
                createdAt: null,
                updatedAt: null,
              });
              setShowMoveModal(true);
            }}
            onDelete={(path) => handleDeleteSingle(path)}
          />
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel defaultSize={80} minSize={50}>
          <ContentPanel
            tabs={tabs}
            currentTab={currentTab}
            onSelectTab={selectTab}
            onCloseTab={closeTab}
            onPinTab={pinTab}
            onReorderTabs={reorderTabs}
            selectedItems={selectedItems}
            onDelete={handleDelete}
            onDownloadSelected={handleDownloadSelected}
            onClearSelection={() => setSelectedItems(new Set())}
            deletePending={deleteMutation.isPending}
            dragOver={dragOver}
            onDrop={handleDrop}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(true);
            }}
            onDragLeave={() => setDragOver(false)}
            isLoading={currentPathQuery.isLoading}
            error={currentPathQuery.error}
            items={currentPathQuery.data || []}
            onToggleSelection={toggleSelection}
            onCopyToClipboard={copyToClipboard}
            onDownload={handleDownload}
            onReplace={handleReplace}
            onDeleteSingle={handleDeleteSingle}
            onOpenPreview={(path, name) => openTab("file", name, path)}
            onOpenFolder={(path, name) => navigateToFolder(path, name)}
            onMove={openMoveModal}
            onRename={handleRename}
            onCreateFolder={() => setIsCreatingFolder(true)}
            fileInputRef={fileInputRef}
            createFolderPending={createFolderMutation.isPending}
            uploadPending={uploadMutation.isPending}
            canNavigateBack={historyIndex > 0}
            canNavigateForward={historyIndex < navigationHistory.length - 1}
            onNavigateBack={handleNavigateBack}
            onNavigateForward={handleNavigateForward}
            draggingItem={draggingItem}
            dropTargetPath={dropTargetPath}
            onDragItemStart={handleDragItemStart}
            onDragItemEnd={handleDragItemEnd}
            onDropOnFolder={handleDropOnFolder}
            onSetDropTarget={setDropTargetPath}
          />
        </ResizablePanel>
      </ResizablePanelGroup>

      <MoveFileModal
        open={showMoveModal}
        onOpenChange={(open) => {
          setShowMoveModal(open);
          if (!open) setItemToMove(null);
        }}
        item={itemToMove}
        folders={treeNodes.filter((n) => n.type === "dir")}
        onSubmit={handleMoveFile}
        isPending={moveMutation.isPending}
      />
    </>
  );
}

function Sidebar({
  searchQuery,
  onSearchChange,
  loadingPaths,
  filteredTreeNodes,
  onOpenFolder,
  onOpenFile,
  onToggleNodeExpanded,
  uploadPending,
  fileInputRef,
  onUpload,
  isCreatingFolder,
  onCreateFolderClick,
  onCreateFolder,
  onCancelCreateFolder,
  createFolderPending,
  currentTab,
  onRename,
  onMove,
  onDelete,
}: {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  loadingPaths: Set<string>;
  filteredTreeNodes: TreeNode[];
  onOpenFolder: (path: string, name: string) => void;
  onOpenFile: (path: string, name: string) => void;
  onToggleNodeExpanded: (path: string) => Promise<void>;
  uploadPending: boolean;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  onUpload: (files: FileList) => void;
  isCreatingFolder: boolean;
  onCreateFolderClick: () => void;
  onCreateFolder: (name: string) => void;
  onCancelCreateFolder: () => void;
  createFolderPending: boolean;
  currentTab: Tab | undefined;
  onRename: (path: string, newName: string) => void;
  onMove: (path: string, name: string, type: "file" | "dir") => void;
  onDelete: (path: string) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { atStart, atEnd } = useScrollFade(scrollRef, "vertical", [
    filteredTreeNodes,
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
          {isCreatingFolder && (
            <NewFolderInlineInput
              existingNames={filteredTreeNodes.map((n) => n.name)}
              onSubmit={onCreateFolder}
              onCancel={onCancelCreateFolder}
              isLoading={createFolderPending}
            />
          )}
          {filteredTreeNodes.map((node) => (
            <TreeNodeItem
              key={node.path}
              node={node}
              depth={0}
              loadingPaths={loadingPaths}
              onOpenFolder={onOpenFolder}
              onOpenFile={onOpenFile}
              onToggle={onToggleNodeExpanded}
              currentTab={currentTab}
              onRename={onRename}
              onMove={onMove}
              onDelete={onDelete}
            />
          ))}
        </div>
      </div>

      <AddMenu
        onCreateFolder={onCreateFolderClick}
        createFolderPending={createFolderPending || isCreatingFolder}
        uploadPending={uploadPending}
        fileInputRef={fileInputRef}
        onUpload={onUpload}
      />
    </div>
  );
}

function NewFolderInlineInput({
  existingNames,
  onSubmit,
  onCancel,
  isLoading,
}: {
  existingNames: string[];
  onSubmit: (name: string) => void;
  onCancel: () => void;
  isLoading: boolean;
}) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const validate = (name: string): string | null => {
    if (!name.trim()) {
      return "Name cannot be empty";
    }
    if (existingNames.some((n) => n.toLowerCase() === name.toLowerCase())) {
      return "A folder with this name already exists";
    }
    return null;
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      const name = value.trim();
      const validationError = validate(name);
      if (validationError) {
        setError(validationError);
      } else {
        setError(null);
        onSubmit(name);
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
    const name = value.trim();
    const validationError = validate(name);
    if (validationError) {
      setError(validationError);
      setTimeout(() => inputRef.current?.focus(), 0);
    } else {
      setError(null);
      onSubmit(name);
    }
  };

  return (
    <div>
      <div
        className={cn([
          "flex items-center gap-1.5 py-1.5 pr-2 pl-3 text-sm",
          error ? "bg-red-50" : "bg-neutral-100",
        ])}
      >
        <FolderPlusIcon className="size-4 shrink-0 text-neutral-400" />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            if (error) setError(null);
          }}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          disabled={isLoading}
          placeholder="folder-name"
          className={cn([
            "flex-1 bg-transparent text-sm outline-hidden",
            error ? "text-red-700" : "text-neutral-600",
            "placeholder:text-neutral-400",
          ])}
        />
      </div>
      {error && (
        <div className="bg-red-50 px-3 py-1 text-xs text-red-600">{error}</div>
      )}
    </div>
  );
}

function AddMenu({
  onCreateFolder,
  createFolderPending,
  uploadPending,
  fileInputRef,
  onUpload,
}: {
  onCreateFolder: () => void;
  createFolderPending: boolean;
  uploadPending: boolean;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  onUpload: (files: FileList) => void;
}) {
  const [showMenu, setShowMenu] = useState(false);

  const handleCreateFolder = () => {
    setShowMenu(false);
    onCreateFolder();
  };

  const handleAddFile = () => {
    setShowMenu(false);
    fileInputRef.current?.click();
  };

  const handleCancel = () => {
    setShowMenu(false);
  };

  const isPending = createFolderPending || uploadPending;

  return (
    <div className="relative p-3">
      {showMenu ? (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => setShowMenu(false)}
          />
          <button
            onClick={handleCreateFolder}
            className={cn([
              "absolute right-3 bottom-27 left-3 z-50",
              "flex h-9 w-auto items-center justify-center gap-2 rounded-full text-sm font-medium",
              "border border-neutral-200 bg-linear-to-b from-white to-neutral-100 text-neutral-700",
              "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            ])}
          >
            <FolderPlusIcon className="size-4" />
            Add Folder
          </button>
          <button
            onClick={handleAddFile}
            className={cn([
              "absolute right-3 bottom-15 left-3 z-50",
              "flex h-9 w-auto items-center justify-center gap-2 rounded-full text-sm font-medium",
              "border border-neutral-200 bg-linear-to-b from-white to-neutral-100 text-neutral-700",
              "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            ])}
          >
            <UploadIcon className="size-4" />
            Add File
          </button>
          <button
            onClick={handleCancel}
            className={cn([
              "flex h-9 w-full items-center justify-center gap-2 rounded-full text-sm font-medium",
              "border border-red-200 bg-linear-to-b from-red-50 to-red-100 text-red-700",
              "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            ])}
          >
            <XIcon className="size-4" />
            Cancel
          </button>
        </>
      ) : (
        <button
          onClick={() => setShowMenu(true)}
          disabled={isPending}
          className={cn([
            "flex h-9 w-full items-center justify-center gap-2 rounded-full text-sm font-medium",
            "border border-neutral-200 bg-linear-to-b from-white to-neutral-100 text-neutral-700",
            "shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            "disabled:opacity-50 disabled:hover:scale-100 disabled:hover:shadow-xs",
          ])}
        >
          {isPending ? <Spinner size={14} /> : <PlusIcon className="size-4" />}
          {createFolderPending
            ? "Creating..."
            : uploadPending
              ? "Uploading..."
              : "Add"}
        </button>
      )}

      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept="image/*,video/*,audio/*"
        className="hidden"
        onChange={(e) => e.target.files && onUpload(e.target.files)}
      />
    </div>
  );
}

function TreeNodeItem({
  node,
  depth,
  loadingPaths,
  onOpenFolder,
  onOpenFile,
  onToggle,
  currentTab,
  onRename,
  onMove,
  onDelete,
}: {
  node: TreeNode;
  depth: number;
  loadingPaths: Set<string>;
  onOpenFolder: (path: string, name: string) => void;
  onOpenFile: (path: string, name: string) => void;
  onToggle: (path: string) => Promise<void>;
  currentTab: Tab | undefined;
  onRename: (path: string, newName: string) => void;
  onMove: (path: string, name: string, type: "file" | "dir") => void;
  onDelete: (path: string) => void;
}) {
  const isFolder = node.type === "dir";
  const isLoading = loadingPaths.has(node.path);
  const isActive = currentTab?.path === node.path;
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(node.name);
  const renameInputRef = useRef<HTMLInputElement>(null);

  const handleDoubleClick = () => {
    if (isFolder) {
      onOpenFolder(node.path, node.name);
    }
  };

  const handleClick = async () => {
    if (isRenaming) return;
    if (isFolder) {
      onOpenFolder(node.path, node.name);
    } else {
      onOpenFile(node.path, node.name);
    }
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const closeContextMenu = () => setContextMenu(null);

  const startRename = () => {
    closeContextMenu();
    setRenameValue(node.name);
    setIsRenaming(true);
    setTimeout(() => {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    }, 0);
  };

  const submitRename = () => {
    const trimmed = renameValue.trim();
    if (trimmed && trimmed !== node.name) {
      onRename(node.path, trimmed);
    }
    setIsRenaming(false);
  };

  const cancelRename = () => {
    setRenameValue(node.name);
    setIsRenaming(false);
  };

  return (
    <div>
      <div
        className={cn([
          "flex cursor-pointer items-center gap-1.5 py-1 pr-2 text-sm",
          "transition-colors hover:bg-neutral-100",
          isActive && "bg-blue-50 text-blue-700",
        ])}
        style={{ paddingLeft: `${depth * 16 + 12}px` }}
        onDoubleClick={handleDoubleClick}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {isLoading ? (
          <Spinner size={14} className="shrink-0" />
        ) : isFolder ? (
          node.expanded ? (
            <FolderOpenIcon
              className={cn([
                "size-4 shrink-0",
                isActive ? "text-blue-500" : "text-neutral-400",
              ])}
            />
          ) : (
            <FolderIcon
              className={cn([
                "size-4 shrink-0",
                isActive ? "text-blue-500" : "text-neutral-400",
              ])}
            />
          )
        ) : (
          <FileIcon
            className={cn([
              "size-4 shrink-0",
              isActive ? "text-blue-500" : "text-neutral-400",
            ])}
          />
        )}
        {isRenaming ? (
          <input
            ref={renameInputRef}
            type="text"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={submitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") submitRename();
              if (e.key === "Escape") cancelRename();
            }}
            onClick={(e) => e.stopPropagation()}
            className="min-w-0 flex-1 rounded border border-blue-500 bg-white px-1 text-sm outline-none"
          />
        ) : (
          <span
            className={cn([
              "truncate",
              isActive ? "text-blue-700" : "text-neutral-700",
            ])}
          >
            {node.name}
          </span>
        )}
      </div>

      {contextMenu && (
        <>
          <div className="fixed inset-0 z-40" onClick={closeContextMenu} />
          <div
            className={cn([
              "fixed z-50 min-w-40 py-1",
              "rounded-xs border border-neutral-200 bg-white shadow-lg",
            ])}
            style={{ left: contextMenu.x, top: contextMenu.y }}
          >
            <button
              onClick={startRename}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <PencilIcon className="size-4" />
              Rename
            </button>
            <button
              onClick={() => {
                closeContextMenu();
                onMove(node.path, node.name, node.type);
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <MoveIcon className="size-4" />
              Move to...
            </button>
            <div className="my-1 border-t border-neutral-200" />
            <button
              onClick={() => {
                closeContextMenu();
                onDelete(node.path);
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 transition-colors hover:bg-neutral-100"
            >
              <Trash2Icon className="size-4" />
              Delete
            </button>
          </div>
        </>
      )}

      {node.expanded && node.children.length > 0 && (
        <div className="ml-5.5 border-l border-neutral-200">
          {node.children.map((child) => (
            <TreeNodeItem
              key={child.path}
              node={child}
              depth={depth + 1}
              loadingPaths={loadingPaths}
              onOpenFolder={onOpenFolder}
              onOpenFile={onOpenFile}
              onToggle={onToggle}
              currentTab={currentTab}
              onRename={onRename}
              onMove={onMove}
              onDelete={onDelete}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ContentPanel({
  tabs,
  currentTab,
  onSelectTab,
  onCloseTab,
  onPinTab,
  onReorderTabs,
  selectedItems,
  onDelete,
  onDownloadSelected,
  onClearSelection,
  deletePending,
  dragOver,
  onDrop,
  onDragOver,
  onDragLeave,
  isLoading,
  error,
  items,
  onToggleSelection,
  onCopyToClipboard,
  onDownload,
  onReplace,
  onDeleteSingle,
  onOpenPreview,
  onOpenFolder,
  onMove,
  onRename,
  onCreateFolder,
  fileInputRef,
  createFolderPending,
  uploadPending,
  canNavigateBack,
  canNavigateForward,
  onNavigateBack,
  onNavigateForward,
  draggingItem,
  dropTargetPath,
  onDragItemStart,
  onDragItemEnd,
  onDropOnFolder,
  onSetDropTarget,
}: {
  tabs: Tab[];
  currentTab: Tab | undefined;
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
  onPinTab: (tabId: string) => void;
  onReorderTabs: (tabs: Tab[]) => void;
  selectedItems: Set<string>;
  onDelete: () => void;
  onDownloadSelected: () => void;
  onClearSelection: () => void;
  deletePending: boolean;
  dragOver: boolean;
  onDrop: (e: React.DragEvent) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  isLoading: boolean;
  error: Error | null;
  items: MediaItem[];
  onToggleSelection: (path: string) => void;
  onCopyToClipboard: (text: string) => void;
  onDownload: (publicUrl: string, filename: string) => void;
  onReplace: (file: File, path: string) => void;
  onDeleteSingle: (path: string) => void;
  onOpenPreview: (path: string, name: string) => void;
  onOpenFolder: (path: string, name: string) => void;
  onMove: (item: MediaItem) => void;
  onRename: (path: string, newName: string) => void;
  onCreateFolder: () => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  createFolderPending: boolean;
  uploadPending: boolean;
  canNavigateBack: boolean;
  canNavigateForward: boolean;
  onNavigateBack: () => void;
  onNavigateForward: () => void;
  draggingItem: MediaItem | null;
  dropTargetPath: string | null;
  onDragItemStart: (item: MediaItem) => void;
  onDragItemEnd: () => void;
  onDropOnFolder: (targetFolderPath: string) => void;
  onSetDropTarget: (path: string | null) => void;
}) {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      {currentTab ? (
        <>
          <div className="flex items-end">
            <TabBar
              tabs={tabs}
              onSelectTab={onSelectTab}
              onCloseTab={onCloseTab}
              onPinTab={onPinTab}
              onReorderTabs={onReorderTabs}
            />
            <div className="flex-1 border-b border-neutral-200" />
          </div>

          <HeaderBar
            currentTab={currentTab}
            selectedItems={selectedItems}
            onDelete={onDelete}
            onDownloadSelected={onDownloadSelected}
            onClearSelection={onClearSelection}
            deletePending={deletePending}
            currentFile={
              currentTab.type === "file"
                ? items.find((i) => i.path === currentTab.path)
                : undefined
            }
            onCopyToClipboard={onCopyToClipboard}
            onDownload={onDownload}
            onReplace={onReplace}
            onDeleteSingle={onDeleteSingle}
            onCreateFolder={onCreateFolder}
            fileInputRef={fileInputRef}
            createFolderPending={createFolderPending}
            uploadPending={uploadPending}
            onOpenFolder={onOpenFolder}
            canNavigateBack={canNavigateBack}
            canNavigateForward={canNavigateForward}
            onNavigateBack={onNavigateBack}
            onNavigateForward={onNavigateForward}
            draggingItem={draggingItem}
            dropTargetPath={dropTargetPath}
            onDropOnFolder={onDropOnFolder}
            onSetDropTarget={onSetDropTarget}
          />

          <div className="flex-1 overflow-hidden">
            {currentTab.type === "folder" ? (
              <FolderView
                dragOver={dragOver}
                onDrop={onDrop}
                onDragOver={onDragOver}
                onDragLeave={onDragLeave}
                isLoading={isLoading}
                error={error}
                items={items}
                selectedItems={selectedItems}
                onToggleSelection={onToggleSelection}
                onCopyToClipboard={onCopyToClipboard}
                onReplace={onReplace}
                onDeleteSingle={onDeleteSingle}
                onOpenPreview={onOpenPreview}
                onOpenFolder={onOpenFolder}
                onMove={onMove}
                onRename={onRename}
                draggingItem={draggingItem}
                dropTargetPath={dropTargetPath}
                onDragItemStart={onDragItemStart}
                onDragItemEnd={onDragItemEnd}
                onDropOnFolder={onDropOnFolder}
                onSetDropTarget={onSetDropTarget}
              />
            ) : (
              <FilePreview
                item={items.find((i) => i.path === currentTab.path)}
              />
            )}
          </div>
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center text-neutral-500">
          <div className="text-center">
            <FolderOpenIcon className="mb-3 size-12" />
            <p className="text-sm">
              Double-click a folder or file in the sidebar to open
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

function TabBar({
  tabs,
  onSelectTab,
  onCloseTab,
  onPinTab,
  onReorderTabs,
}: {
  tabs: Tab[];
  onSelectTab: (tabId: string) => void;
  onCloseTab: (tabId: string) => void;
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
  onPin,
}: {
  tab: Tab;
  onSelect: () => void;
  onClose: () => void;
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

  const isHome = tab.isHome === true;

  const handleAuxClick = (e: React.MouseEvent) => {
    if (e.button === 1 && !isHome) {
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
        {isHome ? (
          <HomeIcon className="size-4 text-neutral-400" />
        ) : tab.type === "folder" ? (
          <FolderIcon className="size-4 text-neutral-400" />
        ) : (
          <FileIcon className="size-4 text-neutral-400" />
        )}
        <span className={cn(["max-w-30 truncate", !tab.pinned && "italic"])}>
          {tab.name}
        </span>
        {!isHome && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onClose();
            }}
            className="rounded p-0.5 transition-colors hover:bg-neutral-200"
          >
            <XIcon className="size-3 text-neutral-500" />
          </button>
        )}
      </div>

      {!isHome && contextMenu && (
        <TabContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onCloseTab={onClose}
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
  onPinTab,
  isPinned,
}: {
  x: number;
  y: number;
  onClose: () => void;
  onCloseTab: () => void;
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
        <button
          onClick={() => {
            onCloseTab();
            onClose();
          }}
          className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
        >
          <XIcon className="size-4" />
          Close
        </button>
        <div className="my-1 border-t border-neutral-200" />
        <button
          onClick={() => {
            onPinTab();
            onClose();
          }}
          className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
        >
          {isPinned ? (
            <>
              <PinOffIcon className="size-4" />
              Unpin tab
            </>
          ) : (
            <>
              <PinIcon className="size-4" />
              Pin tab
            </>
          )}
        </button>
      </div>
    </>
  );
}

function HeaderBar({
  currentTab,
  selectedItems,
  onDelete,
  onDownloadSelected,
  onClearSelection,
  deletePending,
  currentFile,
  onCopyToClipboard,
  onDownload,
  onReplace,
  onDeleteSingle,
  onCreateFolder,
  fileInputRef,
  createFolderPending,
  uploadPending,
  onOpenFolder,
  canNavigateBack,
  canNavigateForward,
  onNavigateBack,
  onNavigateForward,
  draggingItem,
  dropTargetPath,
  onDropOnFolder,
  onSetDropTarget,
}: {
  currentTab: Tab;
  selectedItems: Set<string>;
  onDelete: () => void;
  onDownloadSelected: () => void;
  onClearSelection: () => void;
  deletePending: boolean;
  currentFile?: MediaItem;
  onCopyToClipboard: (text: string) => void;
  onDownload: (publicUrl: string, filename: string) => void;
  onReplace: (file: File, path: string) => void;
  onDeleteSingle: (path: string) => void;
  onCreateFolder: () => void;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
  createFolderPending: boolean;
  uploadPending: boolean;
  onOpenFolder: (path: string, name: string) => void;
  canNavigateBack: boolean;
  canNavigateForward: boolean;
  onNavigateBack: () => void;
  onNavigateForward: () => void;
  draggingItem: MediaItem | null;
  dropTargetPath: string | null;
  onDropOnFolder: (targetFolderPath: string) => void;
  onSetDropTarget: (path: string | null) => void;
}) {
  const replaceFileInputRef = useRef<HTMLInputElement>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const breadcrumbs = currentTab.path ? currentTab.path.split("/") : [];

  return (
    <div className="flex h-10 items-center justify-between border-b border-neutral-200 px-4">
      <div className="flex items-center gap-1 text-sm text-neutral-500">
        <div className="mr-2 flex items-center gap-0.5">
          <button
            type="button"
            onClick={onNavigateBack}
            disabled={!canNavigateBack}
            className={cn([
              "rounded p-1 transition-colors",
              canNavigateBack
                ? "text-neutral-500 hover:bg-neutral-100 hover:text-neutral-700"
                : "cursor-not-allowed text-neutral-300",
            ])}
            title="Back"
          >
            <ChevronLeftIcon className="size-4" />
          </button>
          <button
            type="button"
            onClick={onNavigateForward}
            disabled={!canNavigateForward}
            className={cn([
              "rounded p-1 transition-colors",
              canNavigateForward
                ? "text-neutral-500 hover:bg-neutral-100 hover:text-neutral-700"
                : "cursor-not-allowed text-neutral-300",
            ])}
            title="Forward"
          >
            <ChevronRightIcon className="size-4" />
          </button>
        </div>
        <span
          className={cn([
            "rounded px-1.5 py-0.5 transition-colors",
            draggingItem &&
              dropTargetPath === "" &&
              "bg-blue-100 ring-2 ring-blue-400",
            draggingItem && "cursor-copy",
          ])}
          onDragOver={(e) => {
            if (!draggingItem) return;
            e.preventDefault();
            onSetDropTarget("");
          }}
          onDragLeave={() => onSetDropTarget(null)}
          onDrop={(e) => {
            e.preventDefault();
            onDropOnFolder("");
          }}
        >
          <button
            type="button"
            onClick={() => onOpenFolder("", "Home")}
            className={cn([
              "font-medium text-neutral-700",
              breadcrumbs.length > 0 && "hover:text-neutral-900",
            ])}
          >
            Home
          </button>
        </span>
        {breadcrumbs.map((crumb, index) => {
          const isLast = index === breadcrumbs.length - 1;
          const folderPath = breadcrumbs.slice(0, index + 1).join("/");
          const isDropTarget = draggingItem && dropTargetPath === folderPath;
          return (
            <span key={index} className="flex items-center gap-1">
              <ChevronRightIcon className="size-4 text-neutral-300" />
              {isLast ? (
                <span className="px-1.5 py-0.5 font-medium text-neutral-700">
                  {crumb}
                </span>
              ) : (
                <span
                  className={cn([
                    "rounded px-1.5 py-0.5 transition-colors",
                    isDropTarget && "bg-blue-100 ring-2 ring-blue-400",
                    draggingItem && "cursor-copy",
                  ])}
                  onDragOver={(e) => {
                    if (!draggingItem) return;
                    e.preventDefault();
                    onSetDropTarget(folderPath);
                  }}
                  onDragLeave={() => onSetDropTarget(null)}
                  onDrop={(e) => {
                    e.preventDefault();
                    onDropOnFolder(folderPath);
                  }}
                >
                  <button
                    type="button"
                    onClick={() => onOpenFolder(folderPath, crumb)}
                    className="hover:text-neutral-700"
                  >
                    {crumb}
                  </button>
                </span>
              )}
            </span>
          );
        })}
        {currentFile && (
          <span className="ml-2 text-xs text-neutral-400">
            {formatFileSize(currentFile.size)} • {currentFile.mimeType}
          </span>
        )}
      </div>

      {currentTab.type === "folder" && selectedItems.size === 0 && (
        <div className="relative">
          <button
            ref={addButtonRef}
            onClick={() => setShowAddMenu(!showAddMenu)}
            disabled={createFolderPending || uploadPending}
            className={cn([
              "flex items-center gap-1.5 rounded-xs px-2 py-1.5 font-mono text-xs font-medium",
              "bg-neutral-900 text-white hover:bg-neutral-800",
              "transition-colors disabled:cursor-not-allowed disabled:opacity-50",
            ])}
          >
            <PlusIcon className="size-3" />
            Add
            <ChevronDownIcon className="size-3" />
          </button>

          {showAddMenu && (
            <>
              <div
                className="fixed inset-0 z-40"
                onClick={() => setShowAddMenu(false)}
              />
              <div
                className={cn([
                  "absolute top-full right-0 z-50 mt-1 min-w-40 py-1",
                  "rounded-xs border border-neutral-200 bg-white shadow-lg",
                ])}
              >
                <button
                  onClick={() => {
                    setShowAddMenu(false);
                    fileInputRef.current?.click();
                  }}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
                >
                  <UploadIcon className="size-4" />
                  Add File
                </button>
                <button
                  onClick={() => {
                    setShowAddMenu(false);
                    onCreateFolder();
                  }}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
                >
                  <FolderPlusIcon className="size-4" />
                  Add Folder
                </button>
              </div>
            </>
          )}
        </div>
      )}

      {currentTab.type === "folder" && selectedItems.size > 0 && (
        <div className="flex items-center gap-2">
          <span className="text-sm text-neutral-600">
            {selectedItems.size} selected
          </span>
          <button
            onClick={onDownloadSelected}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-neutral-600"
            title="Download selected"
          >
            <DownloadIcon className="size-4" />
          </button>
          <button
            onClick={onDelete}
            disabled={deletePending}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-red-600 disabled:opacity-50"
            title="Delete selected"
          >
            <Trash2Icon className="size-4" />
          </button>
          <button
            onClick={onClearSelection}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-neutral-600"
            title="Clear selection"
          >
            <XIcon className="size-4" />
          </button>
        </div>
      )}

      {currentTab.type === "file" && currentFile && (
        <div className="flex items-center gap-1">
          <button
            onClick={() => onCopyToClipboard(currentFile.publicUrl)}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-neutral-600"
            title="Copy URL"
          >
            <CopyIcon className="size-4" />
          </button>
          <button
            onClick={() => onDownload(currentFile.publicUrl, currentFile.name)}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-neutral-600"
            title="Download"
          >
            <DownloadIcon className="size-4" />
          </button>
          <button
            onClick={() => replaceFileInputRef.current?.click()}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-neutral-600"
            title="Replace"
          >
            <RefreshCwIcon className="size-4" />
          </button>
          <button
            onClick={() => onDeleteSingle(currentFile.path)}
            className="rounded p-1.5 text-neutral-400 transition-colors hover:text-red-600"
            title="Delete"
          >
            <Trash2Icon className="size-4" />
          </button>
          <input
            ref={replaceFileInputRef}
            type="file"
            accept="image/*,video/*,audio/*"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) {
                onReplace(file, currentFile.path);
                e.target.value = "";
              }
            }}
          />
        </div>
      )}
    </div>
  );
}

function FolderView({
  dragOver,
  onDrop,
  onDragOver,
  onDragLeave,
  isLoading,
  error,
  items,
  selectedItems,
  onToggleSelection,
  onCopyToClipboard,
  onReplace,
  onDeleteSingle,
  onOpenPreview,
  onOpenFolder,
  onMove,
  onRename,
  draggingItem,
  dropTargetPath,
  onDragItemStart,
  onDragItemEnd,
  onDropOnFolder,
  onSetDropTarget,
}: {
  dragOver: boolean;
  onDrop: (e: React.DragEvent) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  isLoading: boolean;
  error: Error | null;
  items: MediaItem[];
  selectedItems: Set<string>;
  onToggleSelection: (path: string) => void;
  onCopyToClipboard: (text: string) => void;
  onReplace: (file: File, path: string) => void;
  onDeleteSingle: (path: string) => void;
  onOpenPreview: (path: string, name: string) => void;
  onOpenFolder: (path: string, name: string) => void;
  onMove: (item: MediaItem) => void;
  onRename: (path: string, newName: string) => void;
  draggingItem: MediaItem | null;
  dropTargetPath: string | null;
  onDragItemStart: (item: MediaItem) => void;
  onDragItemEnd: () => void;
  onDropOnFolder: (targetFolderPath: string) => void;
  onSetDropTarget: (path: string | null) => void;
}) {
  return (
    <div
      className={cn([
        "relative h-full overflow-y-auto p-4",
        dragOver && "bg-blue-50",
      ])}
      onDrop={onDrop}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
      {isLoading ? (
        <div className="flex h-full items-center justify-center text-neutral-500">
          <Spinner size={24} className="mr-2" />
          Loading...
        </div>
      ) : error ? (
        <div className="flex h-full flex-col items-center justify-center text-neutral-500">
          <AlertCircleIcon className="mb-3 size-12 text-red-400" />
          <p className="text-sm text-red-600">Failed to load media</p>
          <p className="mt-1 text-xs text-neutral-400">{error.message}</p>
        </div>
      ) : items.length === 0 ? (
        <div className="flex h-full flex-col items-center justify-center text-neutral-300">
          <FolderOpenIcon className="mb-2 size-10" />
          <p className="text-sm">Empty folder</p>
        </div>
      ) : (
        <div
          className="grid gap-4"
          style={{
            gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))",
          }}
        >
          {items.map((item) => (
            <MediaItemCard
              key={item.path}
              item={item}
              isSelected={selectedItems.has(item.path)}
              onSelect={() => onToggleSelection(item.path)}
              onCopyPath={() => onCopyToClipboard(item.publicUrl)}
              onReplace={(file) => onReplace(file, item.path)}
              onDelete={() => onDeleteSingle(item.path)}
              onOpenPreview={() => onOpenPreview(item.path, item.name)}
              onOpenFolder={() => onOpenFolder(item.path, item.name)}
              onMove={() => onMove(item)}
              onRename={(newName) => onRename(item.path, newName)}
              isDragging={draggingItem?.path === item.path}
              isDropTarget={item.type === "dir" && dropTargetPath === item.path}
              onDragStart={() => onDragItemStart(item)}
              onDragEnd={onDragItemEnd}
              onDropOnFolder={() => onDropOnFolder(item.path)}
              onSetDropTarget={(isOver) =>
                onSetDropTarget(isOver ? item.path : null)
              }
              canDrop={
                item.type === "dir" &&
                draggingItem !== null &&
                draggingItem.path !== item.path
              }
            />
          ))}
        </div>
      )}
    </div>
  );
}

function MediaItemCard({
  item,
  isSelected,
  onSelect,
  onCopyPath,
  onReplace,
  onDelete,
  onOpenPreview,
  onOpenFolder,
  onMove,
  onRename,
  isDragging,
  isDropTarget,
  onDragStart,
  onDragEnd,
  onDropOnFolder,
  onSetDropTarget,
  canDrop,
}: {
  item: MediaItem;
  isSelected: boolean;
  onSelect: () => void;
  onCopyPath: () => void;
  onReplace: (file: File) => void;
  onDelete: () => void;
  onOpenPreview: () => void;
  onOpenFolder: () => void;
  onMove: () => void;
  onRename: (newName: string) => void;
  isDragging: boolean;
  isDropTarget: boolean;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDropOnFolder: () => void;
  onSetDropTarget: (isOver: boolean) => void;
  canDrop: boolean;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const renameInputRef = useRef<HTMLInputElement>(null);
  const [showMenu, setShowMenu] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(item.name);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const closeContextMenu = () => setContextMenu(null);

  const handleReplace = () => {
    fileInputRef.current?.click();
    setShowMenu(false);
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      onReplace(file);
      e.target.value = "";
    }
  };

  const handleCopyPath = () => {
    onCopyPath();
    setShowMenu(false);
  };

  const handleDelete = () => {
    onDelete();
    setShowMenu(false);
  };

  const handleMove = () => {
    onMove();
    setShowMenu(false);
  };

  const startRename = () => {
    setShowMenu(false);
    setRenameValue(item.name);
    setIsRenaming(true);
    setTimeout(() => {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    }, 0);
  };

  const submitRename = () => {
    const trimmed = renameValue.trim();
    if (trimmed && trimmed !== item.name) {
      onRename(trimmed);
    }
    setIsRenaming(false);
  };

  const cancelRename = () => {
    setRenameValue(item.name);
    setIsRenaming(false);
  };

  if (item.type === "dir") {
    return (
      <div
        draggable={!isRenaming}
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          e.dataTransfer.setData("text/plain", item.path);
          onDragStart();
        }}
        onDragEnd={onDragEnd}
        onDragOver={(e) => {
          if (!canDrop) return;
          e.preventDefault();
          e.stopPropagation();
          onSetDropTarget(true);
        }}
        onDragLeave={(e) => {
          e.stopPropagation();
          onSetDropTarget(false);
        }}
        onDrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          onDropOnFolder();
        }}
        className={cn([
          "group relative cursor-pointer overflow-hidden rounded-lg border transition-all",
          isSelected
            ? "border-blue-500 ring-2 ring-blue-500"
            : isDropTarget
              ? "border-blue-400 bg-blue-50 ring-2 ring-blue-400"
              : "border-neutral-200 hover:border-neutral-300 hover:shadow-md",
          isDragging && "opacity-50",
        ])}
        onClick={isRenaming ? undefined : onOpenFolder}
        onContextMenu={handleContextMenu}
      >
        <div
          className={cn([
            "flex aspect-square items-center justify-center",
            isDropTarget ? "bg-blue-100" : "bg-neutral-100",
          ])}
        >
          <FolderIcon
            className={cn([
              "size-12",
              isDropTarget ? "text-blue-500" : "text-neutral-400",
            ])}
          />
        </div>
        <div className="bg-white p-2">
          {isRenaming ? (
            <input
              ref={renameInputRef}
              type="text"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onBlur={submitRename}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitRename();
                if (e.key === "Escape") cancelRename();
              }}
              onClick={(e) => e.stopPropagation()}
              className="w-full rounded border border-blue-500 bg-white px-1 py-0.5 text-sm text-neutral-700 outline-none"
            />
          ) : (
            <p className="truncate text-sm text-neutral-700" title={item.name}>
              {item.name}
            </p>
          )}
        </div>

        <div
          className={cn([
            "absolute top-2 left-2 z-10 transition-opacity",
            isSelected ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          ])}
          onClick={(e) => {
            e.stopPropagation();
            onSelect();
          }}
        >
          <div
            className={cn([
              "flex h-5 w-5 cursor-pointer items-center justify-center rounded shadow-xs",
              isSelected
                ? "bg-blue-500"
                : "border-2 border-neutral-300 bg-white",
            ])}
          >
            {isSelected && <CheckIcon className="size-3 text-white" />}
          </div>
        </div>

        <div
          className="absolute top-2 right-2 z-10 opacity-0 transition-opacity group-hover:opacity-100"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            onClick={() => setShowMenu(!showMenu)}
            className="flex h-6 w-6 items-center justify-center rounded border border-neutral-200 bg-white/90 shadow-xs hover:bg-white"
          >
            <MoreVerticalIcon className="size-4 text-neutral-700" />
          </button>

          {showMenu && (
            <>
              <div
                className="fixed inset-0 z-40"
                onClick={() => setShowMenu(false)}
              />
              <div
                className={cn([
                  "absolute top-full right-0 z-50 mt-1 min-w-40 py-1",
                  "rounded-xs border border-neutral-200 bg-white shadow-lg",
                ])}
              >
                <button
                  onClick={startRename}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
                >
                  <PencilIcon className="size-4" />
                  Rename
                </button>
                <button
                  onClick={handleMove}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
                >
                  <MoveIcon className="size-4" />
                  Move to...
                </button>
                <div className="my-1 border-t border-neutral-200" />
                <button
                  onClick={handleDelete}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 transition-colors hover:bg-neutral-100"
                >
                  <Trash2Icon className="size-4" />
                  Delete
                </button>
              </div>
            </>
          )}
        </div>

        {contextMenu && (
          <>
            <div className="fixed inset-0 z-40" onClick={closeContextMenu} />
            <div
              className={cn([
                "fixed z-50 min-w-40 py-1",
                "rounded-xs border border-neutral-200 bg-white shadow-lg",
              ])}
              style={{ left: contextMenu.x, top: contextMenu.y }}
            >
              <button
                onClick={() => {
                  closeContextMenu();
                  startRename();
                }}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <PencilIcon className="size-4" />
                Rename
              </button>
              <button
                onClick={() => {
                  closeContextMenu();
                  onMove();
                }}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <MoveIcon className="size-4" />
                Move to...
              </button>
              <div className="my-1 border-t border-neutral-200" />
              <button
                onClick={() => {
                  closeContextMenu();
                  onDelete();
                }}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 transition-colors hover:bg-neutral-100"
              >
                <Trash2Icon className="size-4" />
                Delete
              </button>
            </div>
          </>
        )}
      </div>
    );
  }

  const isImage = item.mimeType?.startsWith("image/");
  const isVideo = item.mimeType?.startsWith("video/");
  const isAudio = item.mimeType?.startsWith("audio/");

  return (
    <div
      draggable={!isRenaming}
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", item.path);
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      className={cn([
        "group relative cursor-pointer overflow-hidden rounded-lg border transition-all",
        isSelected
          ? "border-blue-500 ring-2 ring-blue-500"
          : "border-neutral-200 hover:border-neutral-300 hover:shadow-md",
        isDragging && "opacity-50",
      ])}
      onClick={onOpenPreview}
      onContextMenu={handleContextMenu}
    >
      <div className="flex aspect-square items-center justify-center overflow-hidden bg-neutral-100">
        {isImage && item.publicUrl ? (
          <img
            src={item.publicUrl}
            alt={item.name}
            className="h-full w-full object-cover"
            loading="lazy"
          />
        ) : isVideo ? (
          <div className="relative flex h-full w-full items-center justify-center bg-neutral-900">
            <FileIcon className="size-12 text-neutral-400" />
            <span className="absolute right-2 bottom-2 rounded bg-black/60 px-1.5 py-0.5 text-xs text-white">
              Video
            </span>
          </div>
        ) : isAudio ? (
          <div className="relative flex h-full w-full items-center justify-center bg-neutral-900">
            <FileIcon className="size-12 text-neutral-400" />
            <span className="absolute right-2 bottom-2 rounded bg-black/60 px-1.5 py-0.5 text-xs text-white">
              Audio
            </span>
          </div>
        ) : (
          <FileIcon className="size-12 text-neutral-400" />
        )}
      </div>

      <div
        className={cn([
          "absolute top-2 left-2 z-10 transition-opacity",
          isSelected ? "opacity-100" : "opacity-0 group-hover:opacity-100",
        ])}
        onClick={(e) => {
          e.stopPropagation();
          onSelect();
        }}
      >
        <div
          className={cn([
            "flex h-5 w-5 cursor-pointer items-center justify-center rounded shadow-xs",
            isSelected ? "bg-blue-500" : "border-2 border-neutral-300 bg-white",
          ])}
        >
          {isSelected && <CheckIcon className="size-3 text-white" />}
        </div>
      </div>

      <div
        className="absolute top-2 right-2 z-10 opacity-0 transition-opacity group-hover:opacity-100"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={() => setShowMenu(!showMenu)}
          className="flex h-6 w-6 items-center justify-center rounded border border-neutral-200 bg-white/90 shadow-xs hover:bg-white"
        >
          <MoreVerticalIcon className="size-4 text-neutral-700" />
        </button>

        {showMenu && (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => setShowMenu(false)}
            />
            <div
              className={cn([
                "absolute top-full right-0 z-50 mt-1 min-w-40 py-1",
                "rounded-xs border border-neutral-200 bg-white shadow-lg",
              ])}
            >
              <button
                onClick={startRename}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <PencilIcon className="size-4" />
                Rename
              </button>
              <button
                onClick={handleCopyPath}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <CopyIcon className="size-4" />
                Copy URL
              </button>
              <a
                href={item.publicUrl}
                download={item.name}
                onClick={() => setShowMenu(false)}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <DownloadIcon className="size-4" />
                Download
              </a>
              <button
                onClick={handleReplace}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <RefreshCwIcon className="size-4" />
                Replace
              </button>
              <button
                onClick={handleMove}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
              >
                <MoveIcon className="size-4" />
                Move to...
              </button>
              <div className="my-1 border-t border-neutral-200" />
              <button
                onClick={handleDelete}
                className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 transition-colors hover:bg-neutral-100"
              >
                <Trash2Icon className="size-4" />
                Delete
              </button>
            </div>
          </>
        )}
      </div>

      <div className="bg-white p-2">
        {isRenaming ? (
          <input
            ref={renameInputRef}
            type="text"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={submitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") submitRename();
              if (e.key === "Escape") cancelRename();
            }}
            onClick={(e) => e.stopPropagation()}
            className="w-full rounded border border-blue-500 bg-white px-1 py-0.5 text-sm text-neutral-700 outline-none"
          />
        ) : (
          <p className="truncate text-sm text-neutral-700" title={item.name}>
            {item.name}
          </p>
        )}
        <p className="text-xs text-neutral-400">{formatFileSize(item.size)}</p>
      </div>

      <input
        ref={fileInputRef}
        type="file"
        accept="image/*,video/*,audio/*"
        className="hidden"
        onChange={handleFileChange}
      />

      {contextMenu && (
        <>
          <div className="fixed inset-0 z-40" onClick={closeContextMenu} />
          <div
            className={cn([
              "fixed z-50 min-w-40 py-1",
              "rounded-xs border border-neutral-200 bg-white shadow-lg",
            ])}
            style={{ left: contextMenu.x, top: contextMenu.y }}
          >
            <button
              onClick={() => {
                closeContextMenu();
                startRename();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <PencilIcon className="size-4" />
              Rename
            </button>
            <button
              onClick={() => {
                closeContextMenu();
                onCopyPath();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <CopyIcon className="size-4" />
              Copy URL
            </button>
            <a
              href={item.publicUrl}
              download={item.name}
              onClick={closeContextMenu}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <DownloadIcon className="size-4" />
              Download
            </a>
            <button
              onClick={() => {
                closeContextMenu();
                fileInputRef.current?.click();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <RefreshCwIcon className="size-4" />
              Replace
            </button>
            <button
              onClick={() => {
                closeContextMenu();
                onMove();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors hover:bg-neutral-100"
            >
              <MoveIcon className="size-4" />
              Move to...
            </button>
            <div className="my-1 border-t border-neutral-200" />
            <button
              onClick={() => {
                closeContextMenu();
                onDelete();
              }}
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-red-600 transition-colors hover:bg-neutral-100"
            >
              <Trash2Icon className="size-4" />
              Delete
            </button>
          </div>
        </>
      )}
    </div>
  );
}

function FilePreview({ item }: { item: MediaItem | undefined }) {
  if (!item) {
    return (
      <div className="flex h-full items-center justify-center text-neutral-500">
        <p className="text-sm">File not found</p>
      </div>
    );
  }

  const isImage = item.mimeType?.startsWith("image/");
  const isVideo = item.mimeType?.startsWith("video/");
  const isAudio = item.mimeType?.startsWith("audio/");

  return (
    <div
      className="flex h-full flex-1 items-center justify-center overflow-hidden bg-neutral-50 p-4"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      {isImage && (
        <img
          src={item.publicUrl}
          alt={item.name}
          className="max-h-full max-w-full object-scale-down"
        />
      )}
      {isVideo && (
        <video
          src={item.publicUrl}
          controls
          className="max-h-full max-w-full object-contain"
        />
      )}
      {isAudio && (
        <audio src={item.publicUrl} controls className="w-full max-w-md" />
      )}
      {!isImage && !isVideo && !isAudio && (
        <div className="text-center">
          <FileIcon className="mb-4 size-16 text-neutral-400" />
          <p className="text-sm text-neutral-600">{item.name}</p>
          <p className="mt-1 text-xs text-neutral-400">
            {formatFileSize(item.size)}
          </p>
        </div>
      )}
    </div>
  );
}

function MoveFileModal({
  open,
  onOpenChange,
  item,
  folders,
  onSubmit,
  isPending,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  item: MediaItem | null;
  folders: TreeNode[];
  onSubmit: (destinationFolder: string) => void;
  isPending: boolean;
}) {
  const [selectedFolder, setSelectedFolder] = useState<string>("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(selectedFolder);
  };

  const getAllFolders = (
    nodes: TreeNode[],
    prefix = "",
  ): { path: string; name: string }[] => {
    const result: { path: string; name: string }[] = [];
    for (const node of nodes) {
      if (node.type === "dir") {
        const displayName = prefix ? `${prefix}/${node.name}` : node.name;
        result.push({ path: node.path, name: displayName });
        if (node.children) {
          result.push(...getAllFolders(node.children, displayName));
        }
      }
    }
    return result;
  };

  const allFolders = getAllFolders(folders);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Move File</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <div className="py-4">
            {item && (
              <p className="mb-3 text-sm text-neutral-600">
                Moving: <span className="font-medium">{item.name}</span>
              </p>
            )}
            <label className="mb-2 block text-sm font-medium text-neutral-700">
              Destination Folder
            </label>
            <select
              value={selectedFolder}
              onChange={(e) => setSelectedFolder(e.target.value)}
              className="w-full rounded-md border border-neutral-200 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none"
            >
              <option value="">Root (no folder)</option>
              {allFolders.map((folder) => (
                <option key={folder.path} value={folder.path}>
                  {folder.name}
                </option>
              ))}
            </select>
          </div>
          <DialogFooter>
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-md px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isPending}
              className={cn([
                "rounded-md px-4 py-2 text-sm font-medium text-white",
                "bg-blue-500 hover:bg-blue-600",
                "disabled:cursor-not-allowed disabled:opacity-50",
              ])}
            >
              {isPending ? "Moving..." : "Move"}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
