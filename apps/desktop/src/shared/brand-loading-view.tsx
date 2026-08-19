import { cn } from "@anlg/utils";

export function AnarlogMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 1082 784"
      fill="currentColor"
      className={className}
      aria-hidden="true"
    >
      <path d="M600 391.5C600 276.349 506.651 183 391.5 183C276.349 183 183 276.349 183 391.5C183 444.961 199.068 500.988 227.187 541.12C253.747 579.028 288.181 600 333 600C417.378 600 486.223 527.628 495.045 418.497C496.113 405.286 506.745 394.509 520 394.509H596.985C598.648 394.509 600 393.163 600 391.5ZM783 391.5C783 454.618 781.537 512.253 795.278 556.615C801.439 576.506 808.593 585.812 813.92 590.27C816.268 592.234 820.533 595.241 828.701 597.392C841.519 600.767 852.5 610.745 852.5 624V759C852.5 772.255 841.734 783.114 828.52 782.074C777.88 778.088 733.031 761.198 696.486 730.621C672.723 710.738 655.202 687.348 642.198 663.186C632.309 644.812 603.663 641.741 590.781 658.157C532.985 731.805 446.926 783 333 783C217.819 783 131.503 723.472 77.3135 646.13C24.6818 571.012 0 477.038 0 391.5C0 175.281 175.281 0 391.5 0C607.719 0 783 175.281 783 391.5Z" />
      <path d="M1057.5 600.5C1070.75 600.5 1081.5 611.245 1081.5 624.5V759.5C1081.5 772.755 1070.75 783.5 1057.5 783.5H922.5C909.245 783.5 898.5 772.755 898.5 759.5V624.5C898.5 611.245 909.245 600.5 922.5 600.5H1057.5Z" />
    </svg>
  );
}

export function BrandLoadingView() {
  return (
    <div
      role="status"
      aria-label="Loading"
      data-tauri-drag-region
      className={cn([
        "bg-background flex h-screen w-screen items-center justify-center",
      ])}
    >
      <div
        className={cn([
          "relative w-[4.5rem]",
          "drop-shadow-[0_10px_24px_rgba(0,0,0,0.10)]",
          "dark:drop-shadow-[0_12px_28px_rgba(0,0,0,0.45)]",
        ])}
      >
        <AnarlogMark className="text-foreground/20 w-full" />
        <div
          aria-hidden="true"
          className={cn([
            "pointer-events-none absolute inset-0",
            "text-foreground/75 dark:text-foreground/50",
            "[-webkit-mask-image:linear-gradient(105deg,transparent_36%,#000_50%,transparent_64%)]",
            "[mask-image:linear-gradient(105deg,transparent_36%,#000_50%,transparent_64%)]",
            "[-webkit-mask-size:220%_100%]",
            "[mask-size:220%_100%]",
            "animate-logo-shimmer-sweep",
            "motion-reduce:animate-none motion-reduce:opacity-0",
          ])}
        >
          <AnarlogMark className="w-full" />
        </div>
      </div>
    </div>
  );
}
