import { Camera } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { type ChangeEvent, type ReactNode, useRef } from "react";

import { radii } from "@anlg/design-system/tokens.stylex";

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
  sx,
}: {
  src: string;
  size: number;
  sx?: stylex.StyleXStyles;
}) {
  return (
    <img
      src={src}
      alt=""
      draggable={false}
      {...stylex.props([styles.image, styles.imageSize(size), sx])}
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
      data-avatar-upload
      type="button"
      onClick={() => inputRef.current?.click()}
      aria-label={label}
      title={label}
      {...stylex.props(styles.uploadButton)}
    >
      {children}
      <span {...stylex.props(styles.uploadOverlay)}>
        <Camera {...stylex.props(styles.cameraIcon)} />
      </span>
      <input
        ref={inputRef}
        type="file"
        accept="image/*"
        {...stylex.props(styles.fileInput)}
        onChange={(event) => void handleFileChange(event)}
      />
    </button>
  );
}

const styles = stylex.create({
  cameraIcon: {
    height: "1.25rem",
    width: "1.25rem",
  },
  fileInput: {
    display: "none",
  },
  image: {
    borderColor: "rgb(0 0 0 / 0.1)",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.05)",
    flexShrink: 0,
    objectFit: "cover",
  },
  imageSize: (size: number) => ({
    height: size,
    width: size,
  }),
  uploadButton: {
    borderRadius: radii.full,
    cursor: "pointer",
    display: "block",
    flexShrink: 0,
    position: "relative",
  },
  uploadOverlay: {
    alignItems: "center",
    backgroundColor: "rgb(0 0 0 / 0.4)",
    borderRadius: radii.full,
    color: "white",
    display: "flex",
    inset: 0,
    justifyContent: "center",
    opacity: {
      default: 0,
      ":is([data-avatar-upload]:hover *)": 1,
    },
    position: "absolute",
    transitionDuration: "150ms",
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});

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
