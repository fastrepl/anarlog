import { X } from "lucide-react";
import { useTheme } from "next-themes";
import React from "react";
import { Toaster as Sonner, toast as sonnerToast } from "sonner";

export { sonnerToast };

export interface ToastButtonProps {
  label: string;
  onClick: () => void;
  primary?: boolean;
}

export interface CustomToastProps {
  id: string | number;
  title: string;
  content?: React.ReactNode;
  buttons?: ToastButtonProps[];
  dismissible?: boolean;
  children?: React.ReactNode;
  duration?: number;
}

export function CustomToast(props: CustomToastProps) {
  const { id, title, content, buttons = [], dismissible, children } = props;

  return (
    <div className="relative flex flex-col gap-2 p-4">
      {dismissible && (
        <button
          onClick={() => sonnerToast.dismiss(id)}
          className="absolute top-2 right-2 cursor-pointer rounded-full p-1 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-neutral-100"
          aria-label="Dismiss"
        >
          <X size={16} />
        </button>
      )}

      <div className="font-medium">{title}</div>

      {content && <div className="text-sm text-neutral-600">{content}</div>}

      {children}

      {buttons.length > 0 && (
        <div className="mt-2 flex gap-2">
          {buttons.map((button, index) => (
            <button
              key={index}
              onClick={() => {
                button.onClick();
                sonnerToast.dismiss(id);
              }}
              className={
                button.primary
                  ? "rounded-md bg-neutral-800 px-3 py-1.5 text-sm text-white hover:bg-neutral-700"
                  : "rounded-md bg-neutral-200 px-3 py-1.5 text-sm text-neutral-800 hover:bg-neutral-300"
              }
            >
              {button.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function toast(props: CustomToastProps) {
  return sonnerToast.custom(
    (id) => (
      <div className="group overflow-clip">
        <CustomToast
          id={id}
          title={props.title}
          content={props.content}
          buttons={props.buttons}
          dismissible={props.dismissible}
          children={props.children}
        />
      </div>
    ),
    {
      id: props.id,
      duration: props.dismissible === false ? Infinity : props.duration,
    },
  );
}

type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => {
  const { theme = "system" } = useTheme();

  return (
    <Sonner
      theme={theme as ToasterProps["theme"]}
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border group-[.toaster]:border-border group-[.toaster]:shadow-lg group-[.toaster]:rounded-lg group-[.toaster]:overflow-clip group-[.toaster]:w-[300px]",
          description: "group-[.toast]:text-muted-foreground",
          actionButton:
            "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
          cancelButton:
            "group-[.toast]:bg-muted group-[.toast]:text-muted-foreground",
        },
      }}
      {...props}
    />
  );
};

export { Toaster };
