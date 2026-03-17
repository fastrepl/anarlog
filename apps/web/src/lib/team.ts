export const TEAM_MEMBERS = {
  john: {
    id: "john",
    name: "John Jeong",
    email: "john@hyprnote.com",
    avatar: "/api/images/team/john.png",
    role: "Chief Wisdom Seeker",
    bio: "I love designing simple and intuitive user interfaces.",
    links: {
      twitter: "https://x.com/computeless",
      github: "https://github.com/computelesscomputer",
      linkedin: "https://linkedin.com/in/johntopia",
    },
  },
  yujong: {
    id: "yujong",
    name: "Yujong Lee",
    email: "yujonglee@hyprnote.com",
    avatar: "/api/images/team/yujong.png",
    role: "Chief OSS Lover",
    bio: "I am super bullish about open-source software.",
    links: {
      twitter: "https://x.com/yujonglee",
      github: "https://github.com/yujonglee",
      linkedin: "https://linkedin.com/in/yujong1ee",
    },
  },
  harshika: {
    id: "harshika",
    name: "Harshika",
    email: "harshika.alagh@gmail.com",
    avatar: "/api/images/team/harshika.jpeg",
    role: "",
    bio: "",
    links: {},
  },
} as const;

export const FOUNDERS = [TEAM_MEMBERS.john, TEAM_MEMBERS.yujong] as const;

export const AUTHOR_AVATARS: Record<string, string> = Object.fromEntries(
  Object.values(TEAM_MEMBERS).map((m) => [m.name, m.avatar]),
);

export const AUTHORS = Object.values(TEAM_MEMBERS).map((m) => ({
  name: m.name,
  avatar: m.avatar,
}));

export const ADMIN_EMAILS = [
  "yujonglee@hyprnote.com",
  "yujonglee.dev@gmail.com",
  "john@hyprnote.com",
  "marketing@hyprnote.com",
  "harshika.alagh@gmail.com",
  "yunhyungjo@yonsei.ac.kr",
  "goranmoomin@daum.net",
  "artem@hyprnote.com",
  "stua@fastmail.com",
  "thestua@gmail.com",
];

export const TEAM_PHOTOS = [
  { id: "john-1", name: "john-1.jpg", url: "/api/images/team/john-1.jpg" },
  { id: "john-2", name: "john-2.jpg", url: "/api/images/team/john-2.jpg" },
  {
    id: "palo-alto-1",
    name: "palo-alto-1.jpg",
    url: "/api/images/team/palo-alto-1.jpg",
  },
  {
    id: "palo-alto-2",
    name: "palo-alto-2.jpg",
    url: "/api/images/team/palo-alto-2.jpg",
  },
  {
    id: "palo-alto-3",
    name: "palo-alto-3.jpg",
    url: "/api/images/team/palo-alto-3.jpg",
  },
  {
    id: "palo-alto-4",
    name: "palo-alto-4.jpg",
    url: "/api/images/team/palo-alto-4.jpg",
  },
  { id: "sadang", name: "sadang.jpg", url: "/api/images/team/sadang.jpg" },
  { id: "yc-0", name: "yc-0.jpg", url: "/api/images/team/yc-0.jpg" },
  { id: "yc-1", name: "yc-1.jpg", url: "/api/images/team/yc-1.jpg" },
  { id: "yc-2", name: "yc-2.jpg", url: "/api/images/team/yc-2.jpg" },
  {
    id: "yujong-1",
    name: "yujong-1.jpg",
    url: "/api/images/team/yujong-1.jpg",
  },
  {
    id: "yujong-2",
    name: "yujong-2.jpg",
    url: "/api/images/team/yujong-2.jpg",
  },
  {
    id: "yujong-3",
    name: "yujong-3.jpg",
    url: "/api/images/team/yujong-3.jpg",
  },
  {
    id: "yujong-4",
    name: "yujong-4.jpg",
    url: "/api/images/team/yujong-4.jpg",
  },
];
