import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Restaurant Operating System | GOTIGIN",
  description:
    "Download Restaurant Operating System by GOTIGIN — Community Edition for Windows, Linux, and Android. Professional and Enterprise editions coming soon.",
  keywords:
    "Restaurant Operating System, ROS, GOTIGIN, POS, restaurant software, Community Edition",
  authors: [{ name: "GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED" }],
  creator: "GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED",
  publisher: "GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED",
  metadataBase: new URL("https://ros.gotigin.com"),
  openGraph: {
    title: "Restaurant Operating System | GOTIGIN",
    description:
      "Offline-first restaurant operating system. Download Community Edition for Windows, Linux, and Android.",
    url: "https://ros.gotigin.com",
    siteName: "Restaurant Operating System",
    images: [{ url: "/ros-banner.jpg" }],
    type: "website",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <body className="flex min-h-full flex-col bg-background text-foreground">
        {children}
      </body>
    </html>
  );
}
