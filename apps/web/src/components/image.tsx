import type { ImageProps } from "@unpic/react";
import { Image as UnpicImage } from "@unpic/react/base";
import type { ComponentProps } from "react";
import { transform } from "unpic/providers/netlify";

function isGifSource(src: ImageProps["src"]) {
  return (
    typeof src === "string" &&
    (src.startsWith("data:image/gif") || /\.gif(?:$|[?#])/i.test(src))
  );
}

export const Image = ({
  layout = "constrained",
  background,
  objectFit,
  src,
  ...props
}: Partial<ImageProps> &
  Pick<ImageProps, "src" | "alt"> & {
    objectFit?: "contain" | "cover" | "fill" | "none" | "scale-down";
  }) => {
  if (isGifSource(src)) {
    const imgProps = props as ComponentProps<"img">;

    return (
      <img
        {...imgProps}
        src={src}
        alt={props.alt}
        style={{
          objectFit,
          ...(imgProps.style || {}),
        }}
      />
    );
  }

  const isExternalUrl =
    typeof src === "string" &&
    (src.startsWith("http://") || src.startsWith("https://"));

  return (
    <UnpicImage
      {...(props as any)}
      src={src}
      {...(isExternalUrl ? {} : { transformer: transform })}
      layout={layout}
      background={background}
      style={{
        objectFit: objectFit,
        ...((props as any).style || {}),
      }}
    />
  );
};
