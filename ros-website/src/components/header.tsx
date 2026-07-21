"use client";

import { useState, useEffect } from "react";
import Image from "next/image";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { Menu, X, Moon, Sun } from "lucide-react";

const navItems = [
  { title: "Overview", href: "/#overview" },
  { title: "Editions", href: "/#editions" },
  { title: "Download", href: "/#download" },
  { title: "Gotigin", href: "https://gotigin.com", external: true },
];

export function Header() {
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [darkMode, setDarkMode] = useState(true);

  useEffect(() => {
    const prefersLight =
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: light)").matches;
    if (prefersLight) {
      setDarkMode(false);
      document.documentElement.classList.remove("dark");
    } else {
      setDarkMode(true);
      document.documentElement.classList.add("dark");
    }
  }, []);

  const toggleDarkMode = () => {
    setDarkMode((prev) => {
      const next = !prev;
      document.documentElement.classList.toggle("dark", next);
      return next;
    });
  };

  return (
    <header className="sticky top-0 z-50 w-full border-b bg-background/85 backdrop-blur dark:bg-background/85">
      <div className="container mx-auto flex h-16 items-center justify-between px-4">
        <Link href="/" className="flex items-center space-x-3">
          <Image
            src="/logo.png"
            alt="GOTIGIN Logo"
            width={40}
            height={40}
            className="rounded-lg object-contain"
          />
          <div className="flex flex-col leading-none">
            <span className="text-xl font-bold tracking-tight text-foreground">
              ROS
            </span>
            <span className="mt-1 text-[10px] text-muted-foreground">
              by GOTIGIN
            </span>
          </div>
        </Link>

        <nav className="hidden items-center space-x-1 md:flex">
          {navItems.map((item) =>
            item.external ? (
              <a
                key={item.title}
                href={item.href}
                target="_blank"
                rel="noopener noreferrer"
                className="rounded-lg px-3 py-2 text-sm font-medium text-foreground/80 hover:bg-muted"
              >
                {item.title}
              </a>
            ) : (
              <Link
                key={item.title}
                href={item.href}
                className="rounded-lg px-3 py-2 text-sm font-medium text-foreground/80 hover:bg-muted"
              >
                {item.title}
              </Link>
            )
          )}
          <Button
            variant="ghost"
            size="icon"
            onClick={toggleDarkMode}
            className="ml-2 size-10 rounded-full"
            aria-label="Toggle dark mode"
          >
            {darkMode ? <Sun className="size-5" /> : <Moon className="size-5" />}
          </Button>
        </nav>

        <div className="flex items-center space-x-2 md:hidden">
          <Button
            variant="ghost"
            size="icon"
            onClick={toggleDarkMode}
            className="size-10 rounded-full"
            aria-label="Toggle dark mode"
          >
            {darkMode ? <Sun className="size-5" /> : <Moon className="size-5" />}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
            className="size-10 rounded-full"
            aria-label="Toggle menu"
          >
            {isMobileMenuOpen ? <X className="size-5" /> : <Menu className="size-5" />}
          </Button>
        </div>
      </div>

      {isMobileMenuOpen && (
        <div className="border-t bg-background md:hidden">
          <div className="container mx-auto space-y-2 px-4 py-4">
            {navItems.map((item) =>
              item.external ? (
                <a
                  key={item.title}
                  href={item.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="block rounded-lg px-3 py-2 text-sm font-medium hover:bg-muted"
                  onClick={() => setIsMobileMenuOpen(false)}
                >
                  {item.title}
                </a>
              ) : (
                <Link
                  key={item.title}
                  href={item.href}
                  className="block rounded-lg px-3 py-2 text-sm font-medium hover:bg-muted"
                  onClick={() => setIsMobileMenuOpen(false)}
                >
                  {item.title}
                </Link>
              )
            )}
          </div>
        </div>
      )}
    </header>
  );
}

export function Footer() {
  const currentYear = new Date().getFullYear();

  return (
    <footer className="border-t bg-muted">
      <div className="container mx-auto px-4 py-12">
        <div className="grid grid-cols-1 gap-8 md:grid-cols-2">
          <div className="space-y-4">
            <Link href="/" className="flex items-center space-x-3">
              <Image
                src="/logo.png"
                alt="GOTIGIN Logo"
                width={40}
                height={40}
                className="rounded-lg object-contain"
              />
              <div className="flex flex-col leading-none">
                <span className="text-xl font-bold tracking-tight text-foreground">
                  Restaurant Operating System
                </span>
                <span className="mt-1 text-[10px] text-muted-foreground">
                  by GOTIGIN
                </span>
              </div>
            </Link>
            <p className="text-sm text-muted-foreground">
              A complete operating system for restaurants — offline-first,
              secure, and built for daily operations.
            </p>
          </div>

          <div className="space-y-4">
            <h3 className="text-lg font-semibold">Product</h3>
            <ul className="space-y-2">
              <li>
                <Link
                  href="/#editions"
                  className="text-sm text-muted-foreground hover:underline"
                >
                  Editions
                </Link>
              </li>
              <li>
                <Link
                  href="/#download"
                  className="text-sm text-muted-foreground hover:underline"
                >
                  Downloads
                </Link>
              </li>
              <li>
                <a
                  href="https://gotigin.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-muted-foreground hover:underline"
                >
                  GOTIGIN
                </a>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-12 flex flex-col items-center justify-between border-t pt-8 md:flex-row">
          <p className="text-sm text-muted-foreground">
            © {currentYear} GOTIGIN SOFTWARE & HARDWARE PRIVATE LIMITED. All
            rights reserved.
          </p>
        </div>
      </div>
    </footer>
  );
}
