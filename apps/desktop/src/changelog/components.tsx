import { commands as openerCommands } from "@hypr/plugin-opener2";
import { streamdownComponents } from "@hypr/tiptap/shared";
import { cn } from "@hypr/utils";

export const changelogComponents = {
  ...streamdownComponents,
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      className="text-blue-600 underline hover:text-blue-800"
      href={href}
      onClick={(e) => {
        e.preventDefault();
        if (href) {
          void openerCommands.openUrl(href, null);
        }
      }}
    >
      {children}
    </a>
  ),
  banner: ({
    title,
    variant,
    children,
  }: {
    title?: string;
    variant?: string;
    children?: React.ReactNode;
  }) => (
    <div
      className={cn([
        "my-4 rounded-lg border p-4",
        variant === "warning"
          ? "border-amber-300 bg-amber-50 text-amber-900"
          : "border-blue-300 bg-blue-50 text-blue-900",
      ])}
    >
      {title && <div className="mb-1 font-semibold">{title}</div>}
      <div className="text-sm">{children}</div>
    </div>
  ),
};
