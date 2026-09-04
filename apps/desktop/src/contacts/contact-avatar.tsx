import { type ChangeEvent, type ReactNode, useRef } from "react";

import { Camera } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { updateContactAvatar } from "./queries";

export function persistContactAvatar(
  type: "human" | "organization",
  contactId: string,
  avatarDataUrl: string | null,
): void {
  void updateContactAvatar(type, contactId, avatarDataUrl).catch((error) => {
    console.error("[contacts] failed to update contact avatar", error);
  });
}

const AVATAR_RASTER_SIZE = 70;

export function ContactImage({
  src,
  size,
  className,
}: {
  src: string;
  size: number;
  className?: string;
}) {
  return (
    <img
      src={src}
      alt=""
      draggable={false}
      style={{ width: size, height: size }}
      className={cn([
        "shrink-0 rounded-full object-cover",
        "border border-black/10 shadow-xs",
        className,
      ])}
    />
  );
}

export function AvatarUploadButton({
  label,
  onUpload,
  children,
}: {
  label: string;
  onUpload: (dataUrl: string) => void;
  children: ReactNode;
}) {
  const inputRef = useRef<HTMLInputElement>(null);

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      onUpload(await compressAvatarImage(file));
    } catch (error) {
      console.error("[contacts] failed to process avatar image", error);
    }
  };

  return (
    <button
      type="button"
      onClick={() => inputRef.current?.click()}
      aria-label={label}
      title={label}
      className="group relative block shrink-0 cursor-pointer rounded-full"
    >
      {children}
      <span
        className={cn([
          "absolute inset-0 flex items-center justify-center rounded-full",
          "bg-black/40 text-white opacity-0 transition-opacity group-hover:opacity-100",
        ])}
      >
        <Camera className="size-5" />
      </span>
      <input
        ref={inputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(event) => void handleFileChange(event)}
      />
    </button>
  );
}

async function compressAvatarImage(file: File): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const image = await loadImage(url);
    const side = Math.min(image.naturalWidth, image.naturalHeight);
    if (side === 0) throw new Error("image has no pixels");

    const canvas = document.createElement("canvas");
    canvas.width = AVATAR_RASTER_SIZE;
    canvas.height = AVATAR_RASTER_SIZE;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("canvas 2d context unavailable");

    // JPEG has no alpha channel; flatten transparency onto white.
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, AVATAR_RASTER_SIZE, AVATAR_RASTER_SIZE);
    context.imageSmoothingQuality = "high";
    context.drawImage(
      image,
      (image.naturalWidth - side) / 2,
      (image.naturalHeight - side) / 2,
      side,
      side,
      0,
      0,
      AVATAR_RASTER_SIZE,
      AVATAR_RASTER_SIZE,
    );
    return canvas.toDataURL("image/jpeg", 0.85);
  } finally {
    URL.revokeObjectURL(url);
  }
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("failed to load image"));
    image.src = url;
  });
}
