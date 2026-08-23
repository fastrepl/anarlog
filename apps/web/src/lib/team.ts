export const EDITORS = {
  john: {
    id: "john",
    name: "John Jeong",
    email: "founders@char.com",
    avatar: "/api/assets/team/john.png",
    role: "Chief Wisdom Seeker",
    bio: "I love designing simple and intuitive user interfaces.",
    links: {
      twitter: "https://x.com/computeless",
      github: "https://github.com/computelesscomputer",
      linkedin: "https://linkedin.com/in/johntopia",
    },
  },
  artem: {
    id: "artem",
    name: "Artem",
    email: "artem@hyprnote.com",
    avatar: "/team/artem.jpg",
    role: "",
    bio: "",
    links: {
      twitter: "https://x.com/s_II_a",
    },
  },
} as const;

export const MANIFESTO_SIGNERS = [EDITORS.john, EDITORS.artem] as const;

export const AUTHOR_AVATARS: Record<string, string> = Object.fromEntries(
  Object.values(EDITORS).map((m) => [m.name, m.avatar]),
);

export const AUTHORS = Object.values(EDITORS).map((m) => ({
  name: m.name,
  avatar: m.avatar,
}));

export const ADMIN_EMAILS = [
  "john@hyprnote.com",
  "marketing@hyprnote.com",
  "yunhyungjo@yonsei.ac.kr",
  "artem@hyprnote.com",
  "stua@fastmail.com",
  "thestua@gmail.com",
];

export const TEAM_PHOTOS = [
  { id: "john-1", name: "john-1.jpg", url: "/api/assets/team/john-1.jpg" },
  { id: "john-2", name: "john-2.jpg", url: "/api/assets/team/john-2.jpg" },
  {
    id: "palo-alto-1",
    name: "palo-alto-1.jpg",
    url: "/api/assets/team/palo-alto-1.jpg",
  },
  {
    id: "palo-alto-2",
    name: "palo-alto-2.jpg",
    url: "/api/assets/team/palo-alto-2.jpg",
  },
  {
    id: "palo-alto-3",
    name: "palo-alto-3.jpg",
    url: "/api/assets/team/palo-alto-3.jpg",
  },
  {
    id: "palo-alto-4",
    name: "palo-alto-4.jpg",
    url: "/api/assets/team/palo-alto-4.jpg",
  },
  { id: "sadang", name: "sadang.jpg", url: "/api/assets/team/sadang.jpg" },
  { id: "yc-0", name: "yc-0.jpg", url: "/api/assets/team/yc-0.jpg" },
  { id: "yc-1", name: "yc-1.jpg", url: "/api/assets/team/yc-1.jpg" },
  { id: "yc-2", name: "yc-2.jpg", url: "/api/assets/team/yc-2.jpg" },
];
