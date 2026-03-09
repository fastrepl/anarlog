import { cn } from "@hypr/utils";

export type Logo = {
  src: string;
  alt: string;
  width?: number;
  height?: number;
};

type LogoCardProps = React.ComponentProps<"div"> & {
  logo: Logo;
};

function LogoCard({ logo, className, children, ...props }: LogoCardProps) {
  return (
    <div
      className={cn(
        "flex items-center justify-center bg-transparent px-4 py-8 transition-colors hover:bg-neutral-50 md:p-8",
        className,
      )}
      {...props}
    >
      <img
        alt={logo.alt}
        className="pointer-events-none h-5 select-none md:h-6"
        height={logo.height || "auto"}
        src={logo.src}
        width={logo.width || "auto"}
      />
      {children}
    </div>
  );
}

export function LogoCloud() {
  return (
    <div className="relative grid grid-cols-2 md:grid-cols-5">
      <div className="pointer-events-none absolute -top-px left-1/2 w-full -translate-x-1/2 border-t border-neutral-100" />

      <LogoCard
        className="relative border-r border-b border-neutral-100"
        logo={{
          src: "/icons/databricks.svg",
          alt: "Databricks Logo",
        }}
      />

      <LogoCard
        className="border-b border-neutral-100 md:border-r"
        logo={{
          src: "/icons/meta.svg",
          alt: "Meta Logo",
        }}
      />

      <LogoCard
        className="relative border-r border-b border-neutral-100"
        logo={{
          src: "/icons/apple.svg",
          alt: "Apple Logo",
        }}
      />

      <LogoCard
        className="border-b border-neutral-100 md:border-r"
        logo={{
          src: "/icons/richmond_american.svg",
          alt: "Richmond American Logo",
        }}
      />

      <LogoCard
        className="relative border-r border-b border-neutral-100 md:border-r-0"
        logo={{
          src: "/icons/wayfair.svg",
          alt: "Wayfair Logo",
        }}
      />

      <LogoCard
        className="border-b border-neutral-100 md:border-r md:border-b-0"
        logo={{
          src: "/icons/amazon.svg",
          alt: "Amazon Logo",
        }}
      />

      <LogoCard
        className="border-r border-b border-neutral-100 md:border-b-0"
        logo={{
          src: "/icons/palantir.svg",
          alt: "Palantir Logo",
        }}
      />

      <LogoCard
        className="border-b border-neutral-100 md:border-r md:border-b-0"
        logo={{
          src: "/icons/disney.svg",
          alt: "Disney Logo",
        }}
      />

      <LogoCard
        className="border-r border-neutral-100"
        logo={{
          src: "/icons/adobe.svg",
          alt: "Adobe Logo",
        }}
      />

      <LogoCard
        className="border-neutral-100"
        logo={{
          src: "/icons/bain.svg",
          alt: "Bain Logo",
        }}
      />

      <div className="pointer-events-none absolute -bottom-px left-1/2 w-full -translate-x-1/2 border-b border-neutral-100" />
    </div>
  );
}
