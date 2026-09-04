import { Buildings, Camera, CircleNotch, X } from "@phosphor-icons/react";
import { type ChangeEvent, useRef } from "react";

import { cn } from "@anlg/utils";

import { compressWorkspaceLogo } from "./logo";

export function WorkspaceLogoMark({
  logoDataUrl,
  className,
}: {
  logoDataUrl: string | null;
  className?: string;
}) {
  const markClassName = cn(["size-10 rounded-xl", className]);

  if (logoDataUrl) {
    return (
      <img
        src={logoDataUrl}
        alt=""
        draggable={false}
        className={cn([markClassName, "object-cover"])}
      />
    );
  }

  return (
    <div
      className={cn([
        "bg-primary/10 text-primary flex items-center justify-center",
        markClassName,
      ])}
    >
      <Buildings className="size-1/2" />
    </div>
  );
}

export function WorkspaceLogoButton({
  logoDataUrl,
  label,
  removeLabel,
  canManage,
  pending,
  onUpload,
  onRemove,
}: {
  logoDataUrl: string | null;
  label: string;
  removeLabel: string;
  canManage: boolean;
  pending: boolean;
  onUpload: (dataUrl: string) => void;
  onRemove: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const preview = <WorkspaceLogoMark logoDataUrl={logoDataUrl} />;

  if (!canManage) {
    return <div className="shrink-0">{preview}</div>;
  }

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      onUpload(await compressWorkspaceLogo(file));
    } catch (error) {
      console.error("[team] failed to process workspace logo", error);
    }
  };

  return (
    <div className="relative shrink-0">
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        aria-label={label}
        title={label}
        disabled={pending}
        className="group relative block shrink-0 cursor-pointer rounded-xl"
      >
        {preview}
        <span
          className={cn([
            "absolute inset-0 flex items-center justify-center rounded-xl",
            "bg-black/40 text-white transition-opacity",
            pending ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          ])}
        >
          {pending ? (
            <CircleNotch className="size-4 animate-spin" />
          ) : (
            <Camera className="size-4" />
          )}
        </span>
        <input
          ref={inputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={(event) => void handleFileChange(event)}
        />
      </button>
      {logoDataUrl && !pending ? (
        <button
          type="button"
          aria-label={removeLabel}
          title={removeLabel}
          onClick={onRemove}
          className={cn([
            "bg-background text-muted-foreground absolute -top-1 -right-1",
            "flex size-4 items-center justify-center rounded-full border shadow-xs",
          ])}
        >
          <X className="size-2.5" />
        </button>
      ) : null}
    </div>
  );
}
