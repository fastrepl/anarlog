import type { ComponentProps, CSSProperties } from "react";
import { Toaster as Sonner, toast as sonnerToast } from "sonner";

export { sonnerToast };

type ToasterProps = ComponentProps<typeof Sonner>;

const Toaster = ({
  theme = "system",
  position = "bottom-right",
  richColors = true,
  style,
  ...props
}: ToasterProps) => (
  <Sonner
    theme={theme}
    position={position}
    richColors={richColors}
    className="toaster group"
    style={{ "--width": "300px", ...style } as CSSProperties}
    toastOptions={{
      classNames: {
        toast:
          "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border group-[.toaster]:border-border group-[.toaster]:shadow-md group-[.toaster]:rounded-xl group-[.toaster]:overflow-clip group-[.toaster]:w-[300px] group-[.toaster]:p-3.5 group-[.toaster]:gap-3",
        description: "group-[.toast]:text-muted-foreground",
        actionButton:
          "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
        cancelButton:
          "group-[.toast]:bg-transparent group-[.toast]:text-muted-foreground",
      },
    }}
    {...props}
  />
);

export { Toaster };
