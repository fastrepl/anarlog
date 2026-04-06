export type PlatformPreset = {
  id: string;
  label: string;
  width: number;
  height: number;
};

export const platforms: PlatformPreset[] = [
  { id: "x", label: "X (Twitter)", width: 1200, height: 675 },
  { id: "linkedin", label: "LinkedIn", width: 1200, height: 627 },
  { id: "instagram", label: "Instagram", width: 1080, height: 1080 },
  { id: "facebook", label: "Facebook", width: 1200, height: 630 },
  { id: "og", label: "Open Graph", width: 1200, height: 630 },
];
