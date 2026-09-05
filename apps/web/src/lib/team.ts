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
