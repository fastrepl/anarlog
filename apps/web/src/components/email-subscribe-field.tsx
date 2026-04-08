import { useMutation } from "@tanstack/react-query";
import { ArrowRightIcon, MailIcon } from "lucide-react";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { addContact } from "@/functions/loops";

export function EmailSubscribeField({
  className,
  formClassName,
  source = "LANDING_PAGE",
  variant = "footer",
  buttonLabel = "Subscribe",
}: {
  className?: string;
  formClassName?: string;
  source?: string;
  variant?: "footer" | "hero";
  buttonLabel?: string;
}) {
  const [email, setEmail] = useState("");
  const isHeroVariant = variant === "hero";

  const mutation = useMutation({
    mutationFn: async () => {
      await addContact({
        data: {
          email,
          userGroup: "Lead",
          source,
        },
      });
    },
    onSuccess: () => {
      setEmail("");
    },
  });

  return (
    <div className={cn([className])}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (email) {
            mutation.mutate();
          }
        }}
        className={cn([
          isHeroVariant &&
            "border-color-subtle focus-within:border-color-active relative flex items-center rounded-full border-2 transition-all duration-200",
          !isHeroVariant && "border-color-bright border transition-all",
          formClassName,
        ])}
      >
        {isHeroVariant ? (
          <>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Subscribe to updates"
              className="placeholder:text-color-muted flex-1 px-6 py-4 pr-36 text-base"
              disabled={mutation.isPending || mutation.isSuccess}
            />
            <button
              type="submit"
              disabled={!email || mutation.isPending || mutation.isSuccess}
              className="absolute right-1 rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 py-3 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50"
            >
              {mutation.isPending
                ? "Sending..."
                : mutation.isSuccess
                  ? "Sent!"
                  : buttonLabel}
            </button>
          </>
        ) : (
          <div className="relative flex items-center">
            <MailIcon className="text-color-muted absolute left-2.5 size-3.5" />
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Subscribe to updates"
              className={cn([
                "min-w-0 flex-1 py-1.5 pr-2 pl-8 text-sm",
                "placeholder:text-color-muted bg-transparent",
                "focus:outline-none",
              ])}
            />
            <button
              type="submit"
              disabled={!email || mutation.isPending}
              className={cn([
                "shrink-0 px-2 transition-colors focus:outline-none",
                email ? "text-color" : "text-color-muted",
                mutation.isPending && "opacity-50",
              ])}
            >
              <ArrowRightIcon className="size-4" />
            </button>
          </div>
        )}
      </form>

      {isHeroVariant && mutation.isSuccess && (
        <p className="mt-4 text-sm text-green-600">
          Thanks! We'll be in touch soon.
        </p>
      )}
      {isHeroVariant && mutation.isError && (
        <p className="mt-4 text-sm text-red-600">
          Something went wrong. Please try again.
        </p>
      )}

      {!isHeroVariant && mutation.isError && (
        <p className="mt-1 text-xs text-red-500">
          Something went wrong. Please try again.
        </p>
      )}
    </div>
  );
}
