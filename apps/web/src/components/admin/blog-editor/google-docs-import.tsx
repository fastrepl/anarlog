import clsx from "clsx";
import { FileTextIcon, SendIcon } from "lucide-react";
import { useState } from "react";

interface GoogleDocsImportProps {
  onImport: (url: string) => void;
  isLoading?: boolean;
}

export function GoogleDocsImport({
  onImport,
  isLoading,
}: GoogleDocsImportProps) {
  const [url, setUrl] = useState("");

  const handleSubmit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (url.trim() && !isLoading) {
      onImport(url.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      e.stopPropagation();
      handleSubmit(e);
    }
  };

  return (
    <div className="rounded-lg border border-dashed border-neutral-200 bg-neutral-50/50 p-4">
      <form
        onSubmit={handleSubmit}
        className="flex flex-col gap-2"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 text-neutral-400">
          <FileTextIcon className="size-4" />
          <span className="text-sm">Or import from Google Docs</span>
        </div>
        <div className="flex gap-2">
          <input
            type="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Paste Google Docs link..."
            disabled={isLoading}
            className={clsx([
              "flex-1 px-3 py-2 text-sm",
              "rounded-lg border border-neutral-200 bg-white",
              "focus:border-transparent focus:ring-2 focus:ring-blue-500 focus:outline-none",
              "placeholder:text-neutral-300",
              "disabled:cursor-not-allowed disabled:opacity-50",
            ])}
          />
          <button
            type="submit"
            disabled={!url.trim() || isLoading}
            className={clsx([
              "rounded-lg px-3 py-2",
              "bg-neutral-900 text-white",
              "hover:bg-neutral-800",
              "disabled:cursor-not-allowed disabled:opacity-50",
              "transition-colors",
              "flex items-center gap-2",
            ])}
          >
            {isLoading ? (
              <span className="size-4 animate-spin rounded-full border-2 border-white/30 border-t-white" />
            ) : (
              <SendIcon className="size-4" />
            )}
          </button>
        </div>
      </form>
    </div>
  );
}
