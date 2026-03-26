import { useMutation } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  redirect,
  useNavigate,
} from "@tanstack/react-router";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { doUpdatePassword, fetchUser } from "@/functions/auth";

export const Route = createFileRoute("/update-password")({
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async () => {
    const user = await fetchUser();
    if (!user) {
      throw redirect({ to: "/auth/", search: { flow: "web" } });
    }
  },
});

function Component() {
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  const updateMutation = useMutation({
    mutationFn: () => doUpdatePassword({ data: { password } }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (result && "success" in result && result.success) {
        navigate({ to: "/auth/", search: { flow: "web" } });
      }
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");

    if (password !== confirmPassword) {
      setErrorMessage("Passwords do not match");
      return;
    }
    if (password.length < 6) {
      setErrorMessage("Password must be at least 6 characters");
      return;
    }

    updateMutation.mutate();
  };

  return (
    <div
      className={cn([
        "flex min-h-screen items-center justify-center p-4",
        "bg-linear-to-b from-stone-50 via-stone-100/50 to-stone-50",
      ])}
    >
      <div className="mx-auto max-w-md rounded-xs border border-neutral-200 bg-white p-8">
        <div className="mb-8 text-center">
          <div
            className={cn([
              "mx-auto mb-6 size-28",
              "border border-neutral-200 shadow-xl",
              "flex items-center justify-center",
              "rounded-4xl bg-transparent",
            ])}
          >
            <Image
              src="/api/images/hyprnote/icon.png"
              alt="Char"
              width={96}
              height={96}
              className={cn([
                "size-24",
                "rounded-3xl border border-neutral-200",
              ])}
            />
          </div>
          <h1 className="mb-2 font-serif text-3xl text-stone-800">
            Set new password
          </h1>
          <p className="text-sm text-neutral-500">
            Enter your new password below.
          </p>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="New password"
            required
            className={cn([
              "w-full px-4 py-2",
              "rounded-lg border border-neutral-300",
              "text-neutral-700 placeholder:text-neutral-400",
              "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
            ])}
          />
          <input
            type="password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            placeholder="Confirm new password"
            required
            className={cn([
              "w-full px-4 py-2",
              "rounded-lg border border-neutral-300",
              "text-neutral-700 placeholder:text-neutral-400",
              "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
            ])}
          />
          {errorMessage && (
            <p className="text-center text-sm text-red-500">{errorMessage}</p>
          )}
          <button
            type="submit"
            disabled={updateMutation.isPending || !password || !confirmPassword}
            className={cn([
              "w-full cursor-pointer px-4 py-2",
              "border border-neutral-300",
              "rounded-lg font-medium text-neutral-700",
              "hover:bg-neutral-50",
              "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
              "disabled:cursor-not-allowed disabled:opacity-50",
              "transition-colors",
              "flex items-center justify-center gap-2",
            ])}
          >
            {updateMutation.isPending ? "Updating..." : "Update password"}
          </button>
        </form>

        <Link
          to="/auth/"
          search={{ flow: "web" }}
          className="mt-4 flex items-center justify-center gap-1 text-sm text-neutral-500 transition-colors hover:text-neutral-700"
        >
          Back to sign in
        </Link>
      </div>
    </div>
  );
}
