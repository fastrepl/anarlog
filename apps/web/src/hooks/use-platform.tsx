import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";

export type Platform = "mac" | "windows" | "linux" | "mobile" | "unknown";

const PlatformContext = createContext<Platform>("unknown");

export function resolvePlatformFromUserAgent(
  userAgent: string | null | undefined,
): Platform {
  if (!userAgent) {
    return "unknown";
  }

  const normalizedUserAgent = userAgent.toLowerCase();
  const isMobile =
    /mobile|android|iphone|ipad|ipod|blackberry|iemobile|opera mini/i.test(
      normalizedUserAgent,
    );

  if (isMobile) {
    return "mobile";
  }

  if (normalizedUserAgent.includes("mac")) {
    return "mac";
  }

  if (normalizedUserAgent.includes("win")) {
    return "windows";
  }

  if (normalizedUserAgent.includes("linux")) {
    return "linux";
  }

  return "unknown";
}

export function PlatformProvider({
  children,
  initialPlatform,
}: {
  children: ReactNode;
  initialPlatform: Platform;
}) {
  const [platform, setPlatform] = useState(initialPlatform);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const detectedPlatform = resolvePlatformFromUserAgent(
      window.navigator.userAgent,
    );

    setPlatform((currentPlatform) =>
      currentPlatform === detectedPlatform ? currentPlatform : detectedPlatform,
    );
  }, []);

  return (
    <PlatformContext.Provider value={platform}>
      {children}
    </PlatformContext.Provider>
  );
}

export function usePlatform(): Platform {
  return useContext(PlatformContext);
}

export function getPlatformCTA(platform: Platform): {
  label: string;
  action: "download" | "waitlist";
} {
  if (platform === "mac" || platform === "unknown") {
    return { label: "Download", action: "download" };
  }
  return { label: "Join waitlist", action: "waitlist" };
}

export function getHeroCTA(platform: Platform): {
  buttonLabel: string;
  showInput: boolean;
  inputPlaceholder: string;
  subtext: string;
  subtextLink?: string;
} {
  if (platform === "mobile") {
    return {
      buttonLabel: "Remind me",
      showInput: true,
      inputPlaceholder: "Enter your email",
      subtext: "Get an email reminder that you can check later",
    };
  } else if (platform === "windows" || platform === "linux") {
    return {
      buttonLabel: "Join",
      showInput: true,
      inputPlaceholder: "Enter your email",
      subtext:
        "Join the waitlist and get notified as soon as the app is released",
    };
  }

  return {
    buttonLabel: "Download",
    showInput: false,
    inputPlaceholder: "",
    subtext: "Free and open-source",
    subtextLink: "/github",
  };
}
