import { Link, useRouterState } from "@tanstack/react-router";
import { allSolutions } from "content-collections";
import { ExternalLinkIcon, MailIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { EmailSubscribeField } from "@/components/email-subscribe-field";
import { brandPageNoiseBackgroundImage } from "@/lib/brand-noise";

const vsList = [
  { slug: "otter", name: "Otter.ai" },
  { slug: "granola", name: "Granola" },
  { slug: "fireflies", name: "Fireflies" },
  { slug: "fathom", name: "Fathom" },
  { slug: "notion", name: "Notion" },
  { slug: "obsidian", name: "Obsidian" },
];

const useCasesList = allSolutions
  .sort((a, b) => a.order - b.order)
  .map((s) => ({ slug: s.slug, label: s.label.replace(/^For\s+/, "") }));

function getMaxWidthClass(pathname: string): string {
  const isBlogOrDocs =
    pathname.startsWith("/blog") || pathname.startsWith("/docs");
  return isBlogOrDocs ? "max-w-6xl" : "max-w-6xl";
}

export function Footer() {
  const currentYear = new Date().getFullYear();
  const router = useRouterState();
  const maxWidthClass = getMaxWidthClass(router.location.pathname);

  return (
    <footer className="relative overflow-hidden">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-0"
      >
        <div
          className="absolute inset-0"
          style={{
            background:
              "linear-gradient(to bottom, transparent, var(--brand-yellow))",
          }}
        />
        <div
          className="absolute inset-0 opacity-30"
          style={{
            backgroundImage: brandPageNoiseBackgroundImage,
            backgroundRepeat: "repeat",
            maskImage: "linear-gradient(to bottom, transparent, black)",
            WebkitMaskImage: "linear-gradient(to bottom, transparent, black)",
          }}
        />
      </div>
      <div
        aria-hidden="true"
        className="brackets-footer pointer-events-none absolute bottom-0 left-0 z-0 hidden h-full px-8 lg:block"
      >
        <svg
          width="auto"
          height="100%"
          viewBox="0 0 482 1782"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M481.078 253.446C481.078 345.831 432.859 430.23 372.767 502.068C284.229 607.912 230.422 743.313 230.422 890.885C230.422 1038.45 284.231 1173.85 372.768 1279.69C432.861 1351.53 481.078 1435.93 481.078 1528.31V1781.77H181.197V1486.45C181.197 1389.59 132.328 1298.81 50.2617 1243.24L0 1209.2V564.472L50.2616 530.434C132.328 474.856 181.197 384.082 181.197 287.22V0L481.078 0V253.446Z"
            fill="currentColor"
          />
        </svg>
      </div>
      <div
        aria-hidden="true"
        className="brackets-footer pointer-events-none absolute right-0 bottom-0 z-0 hidden h-full px-8 lg:block"
      >
        <svg
          width="auto"
          height="100%"
          viewBox="0 0 482 1782"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M0 253.763C0 346.264 48.3109 430.769 108.519 502.697C197.226 608.673 251.136 744.243 251.136 892C251.136 1039.75 197.224 1175.32 108.517 1281.29C48.3094 1353.22 0 1437.72 0 1530.22V1784H300.456V1488.31C300.456 1391.33 349.418 1300.44 431.642 1244.79L482 1210.71V565.179L431.642 531.098C349.418 475.451 300.456 384.562 300.456 287.579V0L0 0V253.763Z"
            fill="currentColor"
          />
        </svg>
      </div>
      <div
        className={`${maxWidthClass} laptop:px-0 relative z-10 mx-auto px-4 py-12 lg:py-32`}
      >
        <div className="flex flex-col gap-12 lg:flex-row">
          <BrandSection currentYear={currentYear} />
          <LinksGrid />
        </div>
      </div>
    </footer>
  );
}

function BrandSection({ currentYear }: { currentYear: number }) {
  return (
    <div className="lg:flex-1">
      <Link to="/" className="text-fg mb-4 inline-block">
        <svg
          width="auto"
          height="40"
          viewBox="0 0 179 56"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M178.168 10.1559H169.886C165.243 10.1559 161.045 12.6867 158.827 16.6325V10.1559H139.341V17.9748H151.024V46.6134H139.341V54.4323H178.038V46.6134H160.384V27.8669C160.384 22.9751 164.337 19.0162 169.204 19.0162H178.168V10.1559Z"
            fill="currentColor"
          />
          <path
            fill-rule="evenodd"
            clip-rule="evenodd"
            d="M112.196 9.11454C118.644 9.11454 123.929 11.6007 127.637 15.66V10.1559H135.44V54.4323H127.637V48.8167C123.897 52.9366 118.581 55.4737 112.196 55.4737C99.1283 55.4735 90.7706 44.9416 90.7706 32.2938C90.7706 25.9837 92.7715 20.1955 96.4818 15.9659C100.207 11.7191 105.597 9.11462 112.196 9.11454ZM113.235 17.7141C109.058 17.7141 105.751 19.3377 103.473 21.9243C101.181 24.5278 99.8712 28.1785 99.8711 32.2938C99.8711 40.4948 105.2 46.8735 113.235 46.8735C121.269 46.8734 126.598 40.4947 126.598 32.2938C126.598 28.1785 125.289 24.5278 122.996 21.9243C120.718 19.3377 117.411 17.7141 113.235 17.7141Z"
            fill="currentColor"
          />
          <path
            d="M86.8688 27.0861C86.8688 17.7398 79.3141 10.156 69.9865 10.1559H68.0783C63.482 10.156 58.6341 12.6643 56.234 16.7078V0H47.9117V54.4323H57.2722V27.8669C57.2723 25.5333 58.4895 23.3331 60.4315 21.6796C62.3758 20.0242 64.9392 19.0162 67.3976 19.0162H67.7762C72.8912 19.0162 77.5083 23.1922 77.5083 28.1275V54.4323H86.8688V27.0861Z"
            fill="currentColor"
          />
          <path
            d="M34.6029 38.794C33.5438 43.794 29.0311 47.1341 22.8536 47.1341C14.5715 47.1341 9.10115 40.6257 9.10115 32.2938C9.1012 28.1101 10.445 24.3962 12.8028 21.7453C15.1465 19.1103 18.552 17.4541 22.8536 17.4541C29.1178 17.4541 33.5493 21.1411 34.6398 25.8197L34.875 26.8286H44.2661L43.9895 25.292C42.3184 16.0138 34.1545 9.11454 22.8536 9.11454C15.8435 9.11458 10.1045 11.6953 6.12352 15.9328C2.1528 20.1593 4.96314e-05 25.959 0 32.2938C0 45.0293 9.00019 55.4736 22.8536 55.4737C33.8508 55.4737 42.4609 49.4187 43.9704 39.2567L44.1928 37.7596H34.8221L34.6029 38.794Z"
            fill="currentColor"
          />
        </svg>
      </Link>
      <p className="text-fg-muted mb-4 text-sm">Fastrepl © {currentYear}</p>
      <EmailSubscribeField className="mb-4 max-w-72" />

      <p className="text-fg-muted text-sm">
        <Link
          to="/legal/$slug/"
          params={{ slug: "terms" }}
          className="hover:text-color no-underline transition-colors hover:underline hover:decoration-dotted"
        >
          Terms
        </Link>
        {" · "}
        <Link
          to="/legal/$slug/"
          params={{ slug: "privacy" }}
          className="hover:text-color no-underline transition-colors hover:underline hover:decoration-dotted"
        >
          Privacy
        </Link>
      </p>
    </div>
  );
}

function LinksGrid() {
  return (
    <div className="grid grid-cols-2 gap-8 sm:grid-cols-3 lg:shrink-0 lg:grid-cols-5">
      <ProductLinks />
      <ResourcesLinks />
      <CompanyLinks />
      <ToolsLinks />
      <SocialLinks />
    </div>
  );
}

function ProductLinks() {
  return (
    <div>
      <h3 className="text-fg mb-4 font-mono text-sm font-semibold">Product</h3>
      <ul className="flex flex-col gap-3">
        <li>
          <Link
            to="/download/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Download
          </Link>
        </li>
        <li>
          <Link
            to="/changelog/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Changelog
          </Link>
        </li>
        <li>
          <Link
            to="/roadmap/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Roadmap
          </Link>
        </li>
        <li>
          <Link
            to="/docs/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Docs
          </Link>
        </li>
        <li>
          <a
            href="https://github.com/fastrepl/char"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            GitHub
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
        <li>
          <a
            href="https://status.hyprnote.com"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Status
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
      </ul>
    </div>
  );
}

function useRotatingIndex(listLength: number, interval: number) {
  const [index, setIndex] = useState(0);
  const [fading, setFading] = useState(false);
  const pausedRef = useRef(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setIndex(Math.floor(Math.random() * listLength));
  }, [listLength]);

  const advance = useCallback(() => {
    if (pausedRef.current) return;
    setFading(true);
    timeoutRef.current = setTimeout(() => {
      if (pausedRef.current) return;
      setIndex((prev) => (prev + 1) % listLength);
      setFading(false);
    }, 200);
  }, [listLength]);

  useEffect(() => {
    const id = setInterval(advance, interval);
    return () => {
      clearInterval(id);
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [advance, interval]);

  const pause = useCallback(() => {
    pausedRef.current = true;
  }, []);
  const resume = useCallback(() => {
    pausedRef.current = false;
  }, []);

  return { index, fading, pause, resume };
}

function ResourcesLinks() {
  const vs = useRotatingIndex(vsList.length, 3000);
  const useCase = useRotatingIndex(useCasesList.length, 4000);

  const currentVs = vsList[vs.index];
  const currentUseCase = useCasesList[useCase.index];

  return (
    <div>
      <h3 className="text-fg mb-4 font-mono text-sm font-semibold">
        Resources
      </h3>
      <ul className="flex flex-col gap-3">
        <li>
          <Link
            to="/pricing/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Pricing
          </Link>
        </li>
        <li>
          <a
            href="/docs/faq"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            FAQ
          </a>
        </li>
        <li>
          <Link
            to="/company-handbook/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Company Handbook
          </Link>
        </li>
        <li>
          <Link
            to="/gallery/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Prompt Gallery
          </Link>
        </li>
        <li>
          <a
            href="https://github.com/fastrepl/char/discussions"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Discussions
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
        <li>
          <a
            href="mailto:support@hyprnote.com"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Support
            <MailIcon className="size-3" />
          </a>
        </li>
        <li onMouseEnter={useCase.pause} onMouseLeave={useCase.resume}>
          <Link
            to="/solution/$slug/"
            params={{ slug: currentUseCase.slug }}
            className={cn(
              "text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted",
              "inline-flex items-center gap-1",
            )}
            aria-label={`Char for ${currentUseCase.label}`}
          >
            👍 for{" "}
            <span
              className={cn(
                "transition-opacity duration-200",
                useCase.fading ? "opacity-0" : "opacity-100",
              )}
            >
              {currentUseCase.label}
            </span>
          </Link>
        </li>
        <li onMouseEnter={vs.pause} onMouseLeave={vs.resume}>
          <Link
            to="/vs/$slug/"
            params={{ slug: currentVs.slug }}
            className={cn(
              "text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted",
              "inline-flex items-center gap-1",
            )}
            aria-label={`Versus ${currentVs.name}`}
          >
            <img
              src="/api/images/hyprnote/icon.png"
              alt="Char"
              width={12}
              height={12}
              className="inline size-4 rounded border border-neutral-100"
            />{" "}
            vs{" "}
            <span
              className={cn(
                "transition-opacity duration-200",
                vs.fading ? "opacity-0" : "opacity-100",
              )}
            >
              {currentVs.name}
            </span>
          </Link>
        </li>
      </ul>
    </div>
  );
}

function CompanyLinks() {
  return (
    <div>
      <h3 className="text-fg mb-4 font-mono text-sm font-semibold">Company</h3>
      <ul className="flex flex-col gap-3">
        <li>
          <Link
            to="/blog/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Blog
          </Link>
        </li>
        <li>
          <Link
            to="/updates/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Updates
          </Link>
        </li>
        <li>
          <Link
            to="/about/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            About us
          </Link>
        </li>
        {import.meta.env.DEV ? (
          <li>
            <Link
              to="/jobs/"
              className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
            >
              Jobs
            </Link>
          </li>
        ) : null}
        <li>
          <Link
            to="/brand/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Brand
          </Link>
        </li>
        <li>
          <Link
            to="/press-kit/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Press Kit
          </Link>
        </li>
        <li>
          <Link
            to="/opensource/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Open Source
          </Link>
        </li>
      </ul>
    </div>
  );
}

function ToolsLinks() {
  return (
    <div>
      <h3 className="text-fg mb-4 font-mono text-sm font-semibold">Tools</h3>
      <ul className="flex flex-col gap-3">
        <li>
          <Link
            to="/eval/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            AI Eval
          </Link>
        </li>
        <li>
          <Link
            to="/file-transcription/"
            search={{ id: undefined }}
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Audio Transcription
          </Link>
        </li>
        <li>
          <Link
            to="/oss-friends/"
            className="text-fg-muted hover:text-color text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            OSS Navigator
          </Link>
        </li>
      </ul>
    </div>
  );
}

function SocialLinks() {
  return (
    <div>
      <h3 className="text-fg mb-4 font-mono text-sm font-semibold">Social</h3>
      <ul className="flex flex-col gap-3">
        <li>
          <a
            href="/x"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Twitter
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
        <li>
          <a
            href="/discord"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            Discord
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
        <li>
          <a
            href="/youtube"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            YouTube
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
        <li>
          <a
            href="/linkedin"
            target="_blank"
            rel="noopener noreferrer"
            className="text-fg-muted hover:text-color inline-flex items-center gap-1 text-sm no-underline transition-colors hover:underline hover:decoration-dotted"
          >
            LinkedIn
            <ExternalLinkIcon className="size-3" />
          </a>
        </li>
      </ul>
    </div>
  );
}
