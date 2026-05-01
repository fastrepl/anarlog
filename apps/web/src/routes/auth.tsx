import { Icon } from "@iconify-icon/react";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, redirect } from "@tanstack/react-router";
import { ArrowLeftIcon, CheckIcon, CopyIcon, MailIcon } from "lucide-react";
import { useRef, useState } from "react";
import { z } from "zod";

import { cn } from "@hypr/utils";

import {
  createDesktopSession,
  doAuth,
  doMagicLinkAuth,
  doPasswordSignIn,
  doPasswordSignUp,
  exchangeOAuthCode,
  exchangeOtpToken,
  fetchUser,
} from "@/functions/auth";
import { type DesktopScheme, flowSearchSchema } from "@/functions/desktop-flow";
import { useMountEffect } from "@/hooks/useMountEffect";

const commonSearch = {
  redirect: z.string().optional(),
  provider: z.enum(["github", "google"]).optional(),
  rra: z.boolean().optional(),
  code: z.string().optional(),
  token_hash: z.string().optional(),
  type: z
    .enum([
      "email",
      "recovery",
      "magiclink",
      "signup",
      "invite",
      "email_change",
    ])
    .optional(),
  access_token: z.string().optional(),
  refresh_token: z.string().optional(),
  error: z.string().optional(),
  error_description: z.string().optional(),
};

const validateSearch = flowSearchSchema(commonSearch);

export const Route = createFileRoute("/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    if (search.error) {
      return { existingUser: null };
    }

    if (search.flow === "web" && search.code) {
      const result = await exchangeOAuthCode({
        data: { code: search.code, flow: "web" },
      });

      if (result.success) {
        throw redirect({ to: "/" });
      }

      return { existingUser: null };
    }

    if (search.flow === "desktop" && search.code) {
      const result = await exchangeOAuthCode({
        data: { code: search.code, flow: "desktop" },
      });

      if (result.success) {
        throw redirect({
          to: "/auth/",
          search: {
            flow: "desktop",
            scheme: search.scheme,
            access_token: result.access_token,
            refresh_token: result.refresh_token,
          },
        } as any);
      }

      return { existingUser: null };
    }

    if (search.token_hash && search.type) {
      const result = await exchangeOtpToken({
        data: {
          token_hash: search.token_hash,
          type: search.type,
          flow: search.flow,
        },
      });

      if (result.success && search.flow === "desktop") {
        throw redirect({
          to: "/auth/",
          search: {
            flow: "desktop",
            scheme: search.scheme ?? "hyprnote",
            access_token: result.access_token,
            refresh_token: result.refresh_token,
          },
        } as any);
      }

      if (result.success) {
        throw redirect({ to: "/" });
      }
    }

    if (search.access_token && search.refresh_token) {
      return { existingUser: null };
    }

    const user = await fetchUser();

    if (user && search.flow === "web" && !search.provider) {
      throw redirect({ to: "/" });
    }

    if (user && search.flow === "desktop") {
      const result = await createDesktopSession({
        data: { email: user.email },
      });

      if (result) {
        throw redirect({
          to: "/auth/",
          search: {
            flow: "desktop",
            scheme: search.scheme ?? "hyprnote",
            access_token: result.access_token,
            refresh_token: result.refresh_token,
          },
        } as any);
      }
    }

    return { existingUser: user };
  },
});

type AuthView = "main" | "email";
type EmailMode = "password" | "magic-link";

function Component() {
  const search = Route.useSearch();
  const { existingUser } = Route.useRouteContext();
  const [view, setView] = useState<AuthView>("main");

  if (search.error) {
    return (
      <Container>
        <Header title="Sign-in failed" />
        <div className="flex flex-col gap-4 px-8 pb-8">
          <p className="text-center text-sm text-[#5d5549]">
            {search.error_description?.replaceAll("+", " ") ||
              "Something went wrong during sign-in."}
          </p>
          <Link
            to="/auth/"
            search={{ flow: "web" }}
            className="flex w-full items-center justify-center rounded-full bg-[#181613] px-4 py-2 font-sans text-white"
          >
            Try again
          </Link>
        </div>
      </Container>
    );
  }

  if (
    search.flow === "desktop" &&
    search.access_token &&
    search.refresh_token
  ) {
    return (
      <Container>
        <Header title="Sign-in successful" />
        <DesktopTokenView
          scheme={search.scheme ?? "hyprnote"}
          accessToken={search.access_token}
          refreshToken={search.refresh_token}
        />
      </Container>
    );
  }

  if (existingUser && search.flow === "desktop") {
    return (
      <Container>
        <Header title="Welcome back" />
        <DesktopReauthView
          email={existingUser.email}
          scheme={search.scheme ?? "hyprnote"}
        />
      </Container>
    );
  }

  const showGoogle = !search.provider || search.provider === "google";
  const showGithub = !search.provider || search.provider === "github";
  const showEmail = !search.provider;

  return (
    <Container>
      <Header title="Welcome to Anarlog" />
      {view === "main" && (
        <>
          <div className="flex flex-col gap-2 px-8">
            {showGoogle && (
              <OAuthButton
                flow={search.flow}
                scheme={search.scheme}
                provider="google"
                autoStart={!!existingUser && search.provider === "google"}
              />
            )}
            {showGithub && (
              <OAuthButton
                flow={search.flow}
                scheme={search.scheme}
                provider="github"
                rra={search.rra}
                autoStart={!!existingUser && search.provider === "github"}
              />
            )}
            {showEmail && (
              <button
                onClick={() => setView("email")}
                className={cn([
                  "flex w-full cursor-pointer items-center justify-center gap-3 px-4 py-2",
                  "rounded-full border border-[#181613] font-sans text-[#181613]",
                  "transition-colors hover:bg-[#181613]/10",
                  "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
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
          flow={search.flow}
          scheme={search.scheme}
          onBack={() => setView("main")}
        />
      )}
    </Container>
  );
}

function Container({ children }: { children: React.ReactNode }) {
  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f7f2e8] px-4 text-[#181613]">
      <div className="mx-auto w-full max-w-md min-w-[320px] overflow-hidden rounded-lg border border-[#d3c6b1] bg-[#fffaf1] shadow-xl shadow-[#5b4f3d]/10">
        {children}
      </div>
    </main>
  );
}

function Header({ title }: { title: string }) {
  return (
    <div className="mb-8 text-center">
      <div className="mx-auto mb-8 flex items-center justify-between border-b border-[#d3c6b1] p-8">
        <Link to="/" aria-label="Anarlog home">
          <img src="/logo.svg" alt="Anarlog" className="h-8 w-auto" />
        </Link>
        <h1 className="py-4 font-mono text-lg">{title}</h1>
      </div>
    </div>
  );
}

function DesktopTokenView({
  scheme,
  accessToken,
  refreshToken,
}: {
  scheme: DesktopScheme;
  accessToken: string;
  refreshToken: string;
}) {
  const [copied, setCopied] = useState(false);
  const deeplink = `${scheme}://auth/callback?${new URLSearchParams({
    access_token: accessToken,
    refresh_token: refreshToken,
  }).toString()}`;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(deeplink);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-4 px-8 pb-8">
      <p className="text-center text-sm text-[#5d5549]">
        Click the button below to return to the desktop app.
      </p>
      <a
        href={deeplink}
        className="flex w-full items-center justify-center rounded-full bg-[#181613] px-4 py-2 font-sans text-white transition-colors hover:bg-[#373128]"
      >
        Open Anarlog
      </a>
      <button
        onClick={handleCopy}
        className="flex w-full cursor-pointer items-center justify-center gap-2 rounded-full border border-[#d3c6b1] px-4 py-2 text-sm transition-colors hover:bg-[#181613]/10"
      >
        {copied ? (
          <CheckIcon className="size-4" />
        ) : (
          <CopyIcon className="size-4" />
        )}
        {copied ? "Copied" : "Copy URL"}
      </button>
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
      if (!result) {
        return;
      }

      const params = new URLSearchParams();
      params.set("flow", "desktop");
      params.set("scheme", scheme);
      params.set("access_token", result.access_token);
      params.set("refresh_token", result.refresh_token);
      window.location.href = `/auth?${params.toString()}`;
    },
  });

  useMountEffect(() => {
    retryMutation.mutate();
  });

  const hasRetryFailed =
    retryMutation.isError || (retryMutation.isSuccess && !retryMutation.data);

  return (
    <div className="flex flex-col gap-4 p-8">
      {!hasRetryFailed ? (
        <p className="text-center text-sm text-[#5d5549]">
          Signing in as {email}...
        </p>
      ) : (
        <>
          <div className="text-center">
            <p className="mb-1 text-[#5d5549]">Signed in as {email}</p>
            <p className="text-sm text-[#857a6a]">
              Sign in with your provider to continue to the app.
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
    <p className="mt-4 px-8 pb-8 text-center text-xs text-[#756b5d]">
      By signing up, you agree to our terms and privacy policy.
    </p>
  );
}

function EmailAuthView({
  flow,
  scheme,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  onBack: () => void;
}) {
  const [mode, setMode] = useState<EmailMode>("password");

  return (
    <div className="flex flex-col gap-4 px-8">
      <button
        onClick={onBack}
        className="-mt-2 mb-1 flex items-center gap-1 self-start text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
      >
        <ArrowLeftIcon className="size-3.5" />
        Back
      </button>

      <div className="flex gap-1 rounded-full bg-[#ede3d2] p-1">
        <button
          onClick={() => setMode("password")}
          className={cn([
            "flex-1 rounded-full py-1.5 font-sans text-sm font-medium transition-colors",
            mode === "password"
              ? "bg-white text-[#181613] shadow-sm"
              : "text-[#756b5d] hover:text-[#181613]",
          ])}
        >
          Password
        </button>
        <button
          onClick={() => setMode("magic-link")}
          className={cn([
            "flex-1 rounded-full py-1.5 font-sans text-sm font-medium transition-colors",
            mode === "magic-link"
              ? "bg-white text-[#181613] shadow-sm"
              : "text-[#756b5d] hover:text-[#181613]",
          ])}
        >
          Magic Link
        </button>
      </div>

      {mode === "password" ? (
        <PasswordForm flow={flow} scheme={scheme} />
      ) : (
        <MagicLinkForm flow={flow} scheme={scheme} />
      )}

      <LegalText />
    </div>
  );
}

function PasswordForm({
  flow,
  scheme,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
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
        data: { email, password, flow, scheme },
      }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(result.message);
        return;
      }

      if (result?.success && "access_token" in result) {
        handlePasswordSuccess(
          result.access_token,
          result.refresh_token,
          flow,
          scheme,
        );
      }
    },
  });

  const signUpMutation = useMutation({
    mutationFn: () =>
      doPasswordSignUp({
        data: { email, password, flow, scheme },
      }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(result.message);
        return;
      }

      if (result?.success && "needsConfirmation" in result) {
        setSubmitted(true);
        return;
      }

      if (result?.success && "access_token" in result) {
        handlePasswordSuccess(
          result.access_token,
          result.refresh_token,
          flow,
          scheme,
        );
      }
    },
  });

  const isPending = signInMutation.isPending || signUpMutation.isPending;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");

    if (!isSignUp) {
      signInMutation.mutate();
      return;
    }

    if (password !== confirmPassword) {
      setErrorMessage("Passwords do not match");
      return;
    }

    if (password.length < 6) {
      setErrorMessage("Password must be at least 6 characters");
      return;
    }

    signUpMutation.mutate();
  };

  if (submitted) {
    return (
      <div className="rounded-lg border border-[#d3c6b1] bg-[#f7f2e8] p-4 text-center">
        <p className="font-medium text-[#302b24]">Check your email</p>
        <p className="mt-1 text-sm text-[#756b5d]">
          We sent a confirmation link to {email}.
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      <AuthInput
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
        required
      />
      <AuthInput
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
        required
      />
      {isSignUp ? (
        <AuthInput
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm password"
          required
        />
      ) : null}
      {errorMessage ? (
        <p className="text-center text-sm text-red-600">{errorMessage}</p>
      ) : null}
      <button
        type="submit"
        disabled={
          isPending || !email || !password || (isSignUp && !confirmPassword)
        }
        className={cn([
          "flex w-full cursor-pointer items-center justify-center gap-3 px-4 py-2",
          "rounded-full font-sans transition-colors",
          "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
          "disabled:cursor-not-allowed disabled:opacity-50",
          isSignUp
            ? "border border-[#d3c6b1] text-[#181613] hover:bg-[#181613]/10"
            : "bg-[#181613] text-white hover:bg-[#373128]",
        ])}
      >
        {isPending ? "Loading..." : isSignUp ? "Create account" : "Sign in"}
      </button>
      <button
        type="button"
        onClick={() => {
          setIsSignUp(!isSignUp);
          setErrorMessage("");
          setConfirmPassword("");
        }}
        className="self-center text-sm text-[#756b5d] transition-colors hover:text-[#181613] hover:underline"
      >
        {isSignUp
          ? "Already have an account? Sign in"
          : "Don't have an account? Sign up"}
      </button>
    </form>
  );
}

function MagicLinkForm({
  flow,
  scheme,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
}) {
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const magicLinkMutation = useMutation({
    mutationFn: (nextEmail: string) =>
      doMagicLinkAuth({
        data: {
          email: nextEmail,
          flow,
          scheme,
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
      <div className="rounded-lg border border-[#d3c6b1] bg-[#f7f2e8] p-4 text-center">
        <p className="font-medium text-[#302b24]">Check your email</p>
        <p className="mt-1 text-sm text-[#756b5d]">
          We sent a magic link to {email}.
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
      <AuthInput
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Enter your email"
        required
      />
      <button
        type="submit"
        disabled={magicLinkMutation.isPending || !email}
        className={cn([
          "flex w-full cursor-pointer items-center justify-center gap-2 px-4 py-2",
          "rounded-full border border-[#d3c6b1] font-medium text-[#181613]",
          "transition-colors hover:bg-[#181613]/10",
          "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
          "disabled:cursor-not-allowed disabled:opacity-50",
        ])}
      >
        {magicLinkMutation.isPending ? "Sending..." : "Send magic link"}
      </button>
      {magicLinkMutation.isError ? (
        <p className="text-center text-sm text-red-600">
          Failed to send magic link. Please try again.
        </p>
      ) : null}
    </form>
  );
}

function AuthInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={cn([
        "w-full rounded-lg border border-[#d3c6b1] bg-white px-4 py-2",
        "text-[#181613] placeholder:text-[#9b907f]",
        "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
        props.className,
      ])}
    />
  );
}

function handlePasswordSuccess(
  accessToken: string,
  refreshToken: string,
  flow: "desktop" | "web",
  scheme?: DesktopScheme,
) {
  if (flow === "desktop") {
    const params = new URLSearchParams();
    params.set("flow", "desktop");
    if (scheme) params.set("scheme", scheme);
    params.set("access_token", accessToken);
    params.set("refresh_token", refreshToken);
    window.location.href = `/auth?${params.toString()}`;
    return;
  }

  window.location.href = "/";
}

function OAuthButton({
  flow,
  scheme,
  provider,
  rra,
  autoStart = false,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  provider: "google" | "github";
  rra?: boolean;
  autoStart?: boolean;
}) {
  const oauthMutation = useMutation({
    mutationFn: (nextProvider: "google" | "github") =>
      doAuth({
        data: {
          provider: nextProvider,
          flow,
          scheme,
          rra,
        },
      }),
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
      }
    },
  });
  const hasAutoStartedRef = useRef(false);

  useMountEffect(() => {
    if (autoStart && !hasAutoStartedRef.current) {
      hasAutoStartedRef.current = true;
      oauthMutation.mutate(provider);
    }
  });

  return (
    <button
      onClick={() => oauthMutation.mutate(provider)}
      disabled={oauthMutation.isPending}
      className={cn([
        "flex w-full cursor-pointer items-center justify-center gap-3 px-4 py-2",
        "rounded-full border border-[#181613] font-sans text-[#181613]",
        "transition-colors hover:bg-[#181613]/10",
        "focus:ring-2 focus:ring-stone-500 focus:ring-offset-2 focus:outline-hidden",
        "disabled:cursor-not-allowed disabled:opacity-50",
      ])}
    >
      {provider === "google" ? <Icon icon="logos:google-icon" /> : null}
      {provider === "github" ? <Icon icon="logos:github-icon" /> : null}
      Sign in with {provider.charAt(0).toUpperCase() + provider.slice(1)}
    </button>
  );
}
