import { Icon } from "@iconify-icon/react";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, redirect } from "@tanstack/react-router";
import { ArrowLeftIcon, MailIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { z } from "zod";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import {
  createDesktopSession,
  doAuth,
  doMagicLinkAuth,
  doPasswordSignIn,
  doPasswordSignUp,
  fetchUser,
} from "@/functions/auth";
import { type DesktopScheme, flowSearchSchema } from "@/functions/desktop-flow";

const commonSearch = {
  redirect: z.string().optional(),
  provider: z.enum(["github", "google"]).optional(),
  rra: z.boolean().optional(),
};

const validateSearch = flowSearchSchema(commonSearch);

export const Route = createFileRoute("/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    const user = await fetchUser();

    if (user) {
      if (search.flow === "web") {
        throw redirect({ to: search.redirect || "/app/account/" } as any);
      }

      if (search.flow === "desktop") {
        const result = await createDesktopSession({
          data: { email: user.email },
        });

        if (result) {
          throw redirect({
            to: "/callback/auth/",
            search: {
              flow: "desktop",
              scheme: search.scheme ?? "hyprnote",
              access_token: result.access_token,
              refresh_token: result.refresh_token,
            },
          });
        }
      }
    }

    return { existingUser: user };
  },
});

type AuthView = "main" | "email";

function Component() {
  const { flow, scheme, redirect, provider, rra } = Route.useSearch();
  const { existingUser } = Route.useRouteContext();
  const [view, setView] = useState<AuthView>("main");

  if (existingUser && flow === "desktop") {
    return (
      <Container>
        <Header />
        <DesktopReauthView
          email={existingUser.email}
          scheme={scheme ?? "hyprnote"}
        />
      </Container>
    );
  }

  const showGoogle = !provider || provider === "google";
  const showGithub = !provider || provider === "github";
  const showEmail = !provider;

  return (
    <Container>
      <Header />
      {view === "main" && (
        <>
          <div className="flex flex-col gap-2">
            {showGoogle && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="google"
              />
            )}
            {showGithub && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="github"
                rra={rra}
              />
            )}
            {showEmail && (
              <button
                onClick={() => setView("email")}
                className={cn([
                  "w-full cursor-pointer px-4 py-2",
                  "border border-neutral-300",
                  "rounded-lg font-medium text-neutral-700",
                  "hover:bg-neutral-50",
                  "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
                  "transition-colors",
                  "flex items-center justify-center gap-2",
                ])}
              >
                <MailIcon className="size-4" />
                Sign in with Email
              </button>
            )}
          </div>
          <LegalText />
        </>
      )}
      {view === "email" && (
        <EmailAuthView
          flow={flow}
          scheme={scheme}
          redirect={redirect}
          onBack={() => setView("main")}
        />
      )}
    </Container>
  );
}

function Container({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn([
        "flex min-h-screen items-center justify-center p-4",
        "bg-linear-to-b from-stone-50 via-stone-100/50 to-stone-50",
      ])}
    >
      <div className="mx-auto max-w-md rounded-xs border border-neutral-200 bg-white p-8">
        {children}
      </div>
    </div>
  );
}

function Header() {
  return (
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
          className={cn(["size-24", "rounded-3xl border border-neutral-200"])}
        />
      </div>
      <h1 className="mb-2 font-serif text-3xl text-stone-800">
        Welcome to Char
      </h1>
    </div>
  );
}

function DesktopReauthView({
  email,
  scheme,
}: {
  email: string;
  scheme: DesktopScheme;
}) {
  const retryMutation = useMutation({
    mutationFn: () => createDesktopSession({ data: { email } }),
    onSuccess: (result) => {
      if (result) {
        const params = new URLSearchParams();
        params.set("flow", "desktop");
        params.set("scheme", scheme);
        params.set("access_token", result.access_token);
        params.set("refresh_token", result.refresh_token);
        window.location.href = `/callback/auth?${params.toString()}`;
      }
    },
  });

  useEffect(() => {
    retryMutation.mutate();
  }, []);

  const hasRetryFailed =
    retryMutation.isError || (retryMutation.isSuccess && !retryMutation.data);

  return (
    <div className="flex flex-col gap-4">
      {!hasRetryFailed && (
        <div className="text-center">
          <p className="text-neutral-600">Signing in as {email}...</p>
        </div>
      )}
      {hasRetryFailed && (
        <>
          <div className="text-center">
            <p className="mb-1 text-neutral-600">Signed in as {email}</p>
            <p className="text-sm text-neutral-400">
              Sign in with your provider to continue to the app
            </p>
          </div>
          <div className="flex flex-col gap-2">
            <OAuthButton flow="desktop" scheme={scheme} provider="google" />
            <OAuthButton flow="desktop" scheme={scheme} provider="github" />
          </div>
        </>
      )}
    </div>
  );
}

function LegalText() {
  return (
    <p className="mt-4 text-center text-xs text-neutral-500">
      By signing up, you agree to our{" "}
      <a
        href="https://char.com/legal/terms"
        className="underline hover:text-neutral-700"
      >
        Terms of Service
      </a>{" "}
      and{" "}
      <a
        href="https://char.com/legal/privacy"
        className="underline hover:text-neutral-700"
      >
        Privacy Policy
      </a>
      .
    </p>
  );
}

type EmailMode = "password" | "magic-link";

function EmailAuthView({
  flow,
  scheme,
  redirect,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  onBack: () => void;
}) {
  const [mode, setMode] = useState<EmailMode>("password");

  return (
    <div className="flex flex-col gap-4">
      <button
        onClick={onBack}
        className="-mt-2 mb-1 flex items-center gap-1 self-start text-sm text-neutral-500 transition-colors hover:text-neutral-700"
      >
        <ArrowLeftIcon className="size-3.5" />
        Back
      </button>

      <div className="flex gap-1 rounded-lg bg-neutral-100 p-1">
        <button
          onClick={() => setMode("password")}
          className={cn([
            "flex-1 rounded-md py-1.5 text-sm font-medium transition-colors",
            mode === "password"
              ? "bg-white text-neutral-900 shadow-sm"
              : "text-neutral-500 hover:text-neutral-700",
          ])}
        >
          Password
        </button>
        <button
          onClick={() => setMode("magic-link")}
          className={cn([
            "flex-1 rounded-md py-1.5 text-sm font-medium transition-colors",
            mode === "magic-link"
              ? "bg-white text-neutral-900 shadow-sm"
              : "text-neutral-500 hover:text-neutral-700",
          ])}
        >
          Magic Link
        </button>
      </div>

      {mode === "password" && (
        <PasswordForm flow={flow} scheme={scheme} redirect={redirect} />
      )}
      {mode === "magic-link" && (
        <MagicLinkForm flow={flow} scheme={scheme} redirect={redirect} />
      )}

      <LegalText />
    </div>
  );
}

function PasswordForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isSignUp, setIsSignUp] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const signInMutation = useMutation({
    mutationFn: () =>
      doPasswordSignIn({
        data: { email, password, flow, scheme, redirect },
      }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (
        result &&
        "success" in result &&
        result.success &&
        "access_token" in result
      ) {
        handlePasswordSuccess(
          result.access_token as string,
          result.refresh_token as string,
          flow,
          scheme,
          redirect,
        );
      }
    },
  });

  const signUpMutation = useMutation({
    mutationFn: () =>
      doPasswordSignUp({
        data: { email, password, flow, scheme, redirect },
      }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (result && "success" in result && result.success) {
        if ("needsConfirmation" in result && result.needsConfirmation) {
          setSubmitted(true);
          return;
        }
        if ("access_token" in result) {
          handlePasswordSuccess(
            result.access_token as string,
            result.refresh_token as string,
            flow,
            scheme,
            redirect,
          );
        }
      }
    },
  });

  const isPending = signInMutation.isPending || signUpMutation.isPending;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");

    if (isSignUp) {
      if (password !== confirmPassword) {
        setErrorMessage("Passwords do not match");
        return;
      }
      if (password.length < 6) {
        setErrorMessage("Password must be at least 6 characters");
        return;
      }
      signUpMutation.mutate();
    } else {
      signInMutation.mutate();
    }
  };

  if (submitted) {
    return (
      <div className="rounded-lg border border-stone-200 bg-stone-50 p-4 text-center">
        <p className="font-medium text-stone-700">Check your email</p>
        <p className="mt-1 text-sm text-stone-500">
          We sent a confirmation link to {email}
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
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
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
        required
        className={cn([
          "w-full px-4 py-2",
          "rounded-lg border border-neutral-300",
          "text-neutral-700 placeholder:text-neutral-400",
          "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
        ])}
      />
      {isSignUp && (
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm password"
          required
          className={cn([
            "w-full px-4 py-2",
            "rounded-lg border border-neutral-300",
            "text-neutral-700 placeholder:text-neutral-400",
            "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
          ])}
        />
      )}
      {errorMessage && (
        <p className="text-center text-sm text-red-500">{errorMessage}</p>
      )}
      <button
        type="submit"
        disabled={
          isPending || !email || !password || (isSignUp && !confirmPassword)
        }
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
        {isPending ? "Loading..." : isSignUp ? "Create account" : "Sign in"}
      </button>
      <div className="flex flex-col items-center gap-1">
        <button
          type="button"
          onClick={() => {
            setIsSignUp(!isSignUp);
            setErrorMessage("");
            setConfirmPassword("");
          }}
          className="text-sm text-neutral-500 transition-colors hover:text-neutral-700"
        >
          {isSignUp
            ? "Already have an account? Sign in"
            : "Don't have an account? Sign up"}
        </button>
        {!isSignUp && (
          <Link
            to="/reset-password/"
            className="text-sm text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Forgot password?
          </Link>
        )}
      </div>
    </form>
  );
}

function handlePasswordSuccess(
  accessToken: string,
  refreshToken: string,
  flow: "desktop" | "web",
  scheme?: DesktopScheme,
  redirectPath?: string,
) {
  if (flow === "desktop") {
    const params = new URLSearchParams();
    params.set("flow", "desktop");
    if (scheme) params.set("scheme", scheme);
    params.set("access_token", accessToken);
    params.set("refresh_token", refreshToken);
    window.location.href = `/callback/auth?${params.toString()}`;
  } else {
    window.location.href = redirectPath || "/app/account/";
  }
}

function MagicLinkForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const magicLinkMutation = useMutation({
    mutationFn: (email: string) =>
      doMagicLinkAuth({
        data: {
          email,
          flow,
          scheme,
          redirect,
        },
      }),
    onSuccess: (result) => {
      if (result && !("error" in result)) {
        setSubmitted(true);
      }
    },
  });

  if (submitted) {
    return (
      <div className="rounded-lg border border-stone-200 bg-stone-50 p-4 text-center">
        <p className="font-medium text-stone-700">Check your email</p>
        <p className="mt-1 text-sm text-stone-500">
          We sent a magic link to {email}
        </p>
      </div>
    );
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (email) {
          magicLinkMutation.mutate(email);
        }
      }}
      className="flex flex-col gap-3"
    >
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Enter your email"
        required
        className={cn([
          "w-full px-4 py-2",
          "rounded-lg border border-neutral-300",
          "text-neutral-700 placeholder:text-neutral-400",
          "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
        ])}
      />
      <button
        type="submit"
        disabled={magicLinkMutation.isPending || !email}
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
        {magicLinkMutation.isPending ? "Sending..." : "Send magic link"}
      </button>
      {magicLinkMutation.isError && (
        <p className="text-center text-sm text-red-500">
          Failed to send magic link. Please try again.
        </p>
      )}
    </form>
  );
}

function OAuthButton({
  flow,
  scheme,
  redirect,
  provider,
  rra,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  provider: "google" | "github";
  rra?: boolean;
}) {
  const oauthMutation = useMutation({
    mutationFn: (provider: "google" | "github") =>
      doAuth({
        data: {
          provider,
          flow,
          scheme,
          redirect,
          rra,
        },
      }),
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
      }
    },
  });
  return (
    <button
      onClick={() => oauthMutation.mutate(provider)}
      disabled={oauthMutation.isPending}
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
      {provider === "google" && <Icon icon="logos:google-icon" />}
      {provider === "github" && <Icon icon="logos:github-icon" />}
      Sign in with {provider.charAt(0).toUpperCase() + provider.slice(1)}
    </button>
  );
}
