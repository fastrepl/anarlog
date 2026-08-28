import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { fetchUser } from "@/functions/auth";
const styles = stylex.create({
  style1: {
    marginInline: "auto",
    display: "flex",
    width: "100%",
    maxWidth: "72rem",
    flexDirection: {
      default: "column",
      "@media (width >= 40rem)": "row",
    },
    gap: "2.5rem",
    paddingInline: {
      default: "1.25rem",
      "@media (width >= 48rem)": "2rem",
    },
    paddingBlock: "2.5rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#4f4940",
    justifyContent: {
      default: null,
      "@media (width >= 40rem)": "space-between",
    },
  },
  style2: {
    display: "flex",
    flexDirection: "column",
    gap: ".625rem",
    alignSelf: "flex-start",
  },
  style3: {
    display: "inline-flex",
    alignItems: "center",
    gap: ".375rem",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#756b5d",
    opacity: {
      default: 0.75,
      ":hover": 1,
    },
    transitionProperty: "opacity",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style4: {
    height: "1rem",
    width: "auto",
  },
  style5: {
    width: "fit-content",
    color: {
      default: null,
      ":hover": "#181613",
    },
  },
  style6: {
    display: "grid",
    gridTemplateColumns: {
      default: "repeat(2, minmax(0, 1fr))",
      "@media (width >= 40rem)": "repeat(3, minmax(0, 1fr))",
    },
    columnGap: {
      default: "3rem",
      "@media (width >= 40rem)": "4rem",
    },
    rowGap: "2rem",
  },
  style7: {
    display: "flex",
    flexDirection: "column",
    gap: ".625rem",
  },
  style8: {
    letterSpacing: ".04em",
    color: "#756b5d",
  },
});
const footerGroups = [
  {
    title: "Product",
    links: [
      {
        label: "Download",
        to: "/download/",
      },
      {
        label: "Enterprise",
        to: "/enterprise/",
      },
      {
        label: "Blog",
        to: "/blog/",
      },
      {
        label: "Changelog",
        to: "/changelog/",
      },
      {
        label: "Docs",
        href: "https://docs.anarlog.so",
      },
      {
        label: "Status",
        href: "https://status.anarlog.so",
      },
    ],
  },
  {
    title: "Community",
    links: [
      {
        label: "GitHub",
        href: "https://github.com/fastrepl/anarlog",
      },
      {
        label: "X",
        href: "https://x.com/anarlogapp",
      },
      {
        label: "Discord",
        to: "/discord/",
      },
      {
        label: "Reddit",
        href: "https://www.reddit.com/r/anarlog/",
      },
    ],
  },
  {
    title: "Legal",
    links: [
      {
        label: "Privacy",
        to: "/privacy/",
      },
      {
        label: "Terms",
        to: "/terms/",
      },
    ],
  },
];
export function SiteFooter() {
  const { data: user } = useQuery({
    queryKey: ["auth-user"],
    queryFn: () => fetchUser(),
  });
  return (
    <footer {...stylex.props(styles.style1)}>
      <div {...stylex.props(styles.style2)}>
        <a
          href="https://fastrepl.com"
          target="_blank"
          rel="noopener noreferrer"
          {...stylex.props(styles.style3)}
        >
          <img
            src="/icons/fastrepl.svg"
            alt="Fastrepl"
            width={4261}
            height={1242}
            {...stylex.props(styles.style4)}
          />
          <span>© 2026</span>
        </a>
        {user ? (
          <Link to="/app/account/" {...stylex.props(styles.style5)}>
            Account
          </Link>
        ) : (
          <Link
            to="/auth/"
            search={{
              flow: "web",
            }}
            {...stylex.props(styles.style5)}
          >
            Get started
          </Link>
        )}
      </div>
      <nav {...stylex.props(styles.style6)}>
        {footerGroups.map((group) => (
          <div key={group.title} {...stylex.props(styles.style7)}>
            <span {...stylex.props(styles.style8)}>{group.title}</span>
            {group.links.map((link) =>
              link.to ? (
                <Link
                  key={link.label}
                  to={link.to}
                  {...stylex.props(styles.style5)}
                >
                  {link.label}
                </Link>
              ) : (
                <a
                  key={link.label}
                  href={link.href}
                  {...stylex.props(styles.style5)}
                >
                  {link.label}
                </a>
              ),
            )}
          </div>
        ))}
      </nav>
    </footer>
  );
}
