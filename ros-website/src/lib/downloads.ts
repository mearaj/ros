export type PlatformId = "linux" | "windows" | "android" | "macos" | "ios";

export type PlatformAsset = {
  id: PlatformId;
  name: string;
  status: "available" | "planned";
  binaryUrl: string;
  signatureUrl: string;
  filename: string;
  signatureFilename: string;
};

export const productRelease = {
  name: "Restaurant Operating System",
  version: "0.1.0",
  description:
    "One installer for every supported device. Your edition and plan are applied automatically during setup.",
};

/**
 * Platform-specific binaries live in public/downloads/.
 * The website offers a single product download; the correct file is selected for the visitor's platform.
 */
export const platformAssets: PlatformAsset[] = [
  {
    id: "linux",
    name: "Linux",
    status: "available",
    binaryUrl: "/downloads/ros-linux-x86_64.AppImage",
    signatureUrl: "/downloads/ros-linux-x86_64.AppImage.sig",
    filename: "ros-linux-x86_64.AppImage",
    signatureFilename: "ros-linux-x86_64.AppImage.sig",
  },
  {
    id: "windows",
    name: "Windows",
    status: "available",
    binaryUrl: "/downloads/ros-windows-x64.exe",
    signatureUrl: "/downloads/ros-windows-x64.exe.sig",
    filename: "ros-windows-x64.exe",
    signatureFilename: "ros-windows-x64.exe.sig",
  },
  {
    id: "android",
    name: "Android",
    status: "available",
    binaryUrl: "/downloads/ros-android.apk",
    signatureUrl: "/downloads/ros-android.apk.sig",
    filename: "ros-android.apk",
    signatureFilename: "ros-android.apk.sig",
  },
  {
    id: "macos",
    name: "macOS",
    status: "planned",
    binaryUrl: "",
    signatureUrl: "",
    filename: "",
    signatureFilename: "",
  },
  {
    id: "ios",
    name: "iOS",
    status: "planned",
    binaryUrl: "",
    signatureUrl: "",
    filename: "",
    signatureFilename: "",
  },
];

export function getPlatformAsset(id: PlatformId): PlatformAsset | undefined {
  return platformAssets.find((platform) => platform.id === id);
}

export type Edition = {
  id: string;
  name: string;
  tagline: string;
  priceLabel: string;
  status: "available" | "coming-soon";
  highlights: string[];
};

export const editions: Edition[] = [
  {
    id: "community",
    name: "Community Edition",
    tagline: "Free forever for single-branch restaurants.",
    priceLabel: "Free",
    status: "available",
    highlights: [
      "Single branch",
      "Offline-first POS, KDS & inventory",
      "Unlimited menu items, orders & tables",
      "Local database — your data stays with you",
    ],
  },
  {
    id: "professional",
    name: "Professional Edition",
    tagline: "Multi-branch cloud sync and owner dashboard.",
    priceLabel: "Coming soon",
    status: "coming-soon",
    highlights: [
      "Up to 5 branches",
      "Cloud synchronization",
      "Central owner dashboard",
      "Automatic backups & priority support",
    ],
  },
  {
    id: "enterprise",
    name: "Enterprise Edition",
    tagline: "Scale, support, and deployment tailored to you.",
    priceLabel: "Paid · Coming soon",
    status: "coming-soon",
    highlights: [
      "Entitlement-based branch capacity",
      "Organization analytics",
      "Dedicated support & SLA options",
      "Custom integrations by contract",
    ],
  },
];
