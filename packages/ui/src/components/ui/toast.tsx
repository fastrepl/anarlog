import type { ComponentProps } from "react";
import { Toaster as Sonner, toast as sonnerToast } from "sonner";

export { sonnerToast };

type ToasterProps = ComponentProps<typeof Sonner>;

const Toaster = ({
  theme = "system",
  position = "bottom-right",
  ...props
}: ToasterProps) => (
  <Sonner
    theme={theme}
    position={position}
    className="toaster group"
    toastOptions={{
      classNames: {
        toast:
          "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border group-[.toaster]:border-border group-[.toaster]:shadow-lg group-[.toaster]:rounded-lg group-[.toaster]:overflow-clip group-[.toaster]:w-[300px]",
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
