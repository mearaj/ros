import Image from "next/image";
import Link from "next/link";
import { Header, Footer } from "@/components/header";
import { ProductDownload } from "@/components/product-download";
import { buttonVariants } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { editions } from "@/lib/downloads";
import { cn } from "@/lib/utils";

export default function Home() {
  return (
    <div className="flex min-h-screen flex-col bg-background">
      <Header />

      <section id="overview" className="bg-background">
        <div className="w-full">
          <Image
            src="/ros-banner.jpg"
            alt="Restaurant Operating System"
            width={1024}
            height={682}
            priority
            className="h-auto w-full"
            sizes="100vw"
          />
        </div>

        <div className="bg-background">
          <div className="container mx-auto px-4 py-12 md:py-16">
            <div className="mx-auto max-w-3xl space-y-6 text-center">
              <p className="text-sm font-semibold tracking-[0.2em] text-muted-foreground uppercase">
                GOTIGIN
              </p>
              <h1 className="text-4xl font-bold tracking-tight md:text-5xl lg:text-6xl">
                Restaurant Operating System
              </h1>
              <p className="mx-auto max-w-2xl text-lg text-muted-foreground md:text-xl">
                Not just a POS — a complete offline-first system for daily
                restaurant operations. Secure, fast, and built to grow with you.
              </p>
              <div className="flex flex-col justify-center gap-3 pt-2 sm:flex-row">
                <Link
                  href="/#download"
                  className={cn(
                    buttonVariants({ size: "lg" }),
                    "h-12 px-8 text-lg"
                  )}
                >
                  Download
                </Link>
                <Link
                  href="/#editions"
                  className={cn(
                    buttonVariants({ size: "lg", variant: "outline" }),
                    "h-12 px-8 text-lg"
                  )}
                >
                  View Editions
                </Link>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section id="editions" className="bg-background py-20 md:py-24">
        <div className="container mx-auto px-4">
          <div className="mb-14 space-y-4 text-center">
            <h2 className="text-3xl font-bold tracking-tight md:text-4xl">
              Editions
            </h2>
            <p className="mx-auto max-w-2xl text-lg text-muted-foreground">
              One app for every plan. Your edition is applied automatically
              during setup — no separate installers.
            </p>
          </div>

          <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
            {editions.map((edition) => (
              <Card
                key={edition.id}
                className={`h-full transition-shadow duration-300 hover:shadow-lg ${
                  edition.status === "available" ? "ring-2 ring-primary/30" : ""
                }`}
              >
                <CardHeader>
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <Badge
                      variant={
                        edition.status === "available" ? "default" : "secondary"
                      }
                    >
                      {edition.status === "available"
                        ? "Available now"
                        : "Coming soon"}
                    </Badge>
                    <span className="text-sm font-medium text-muted-foreground">
                      {edition.priceLabel}
                    </span>
                  </div>
                  <CardTitle className="text-xl">{edition.name}</CardTitle>
                  <CardDescription>{edition.tagline}</CardDescription>
                </CardHeader>
                <CardContent>
                  <ul className="space-y-2 text-sm text-muted-foreground">
                    {edition.highlights.map((item) => (
                      <li key={item} className="flex gap-2">
                        <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-primary" />
                        <span>{item}</span>
                      </li>
                    ))}
                  </ul>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </section>

      <section id="download" className="bg-muted py-20 md:py-24">
        <div className="container mx-auto px-4">
          <div className="mb-14 space-y-4 text-center">
            <h2 className="text-3xl font-bold tracking-tight md:text-4xl">
              Download
            </h2>
            <p className="mx-auto max-w-2xl text-lg text-muted-foreground">
              Get Restaurant Operating System for your device. Setup walks you
              through activation and applies the right plan for your restaurant.
            </p>
          </div>
          <ProductDownload />
        </div>
      </section>

      <section className="bg-primary py-20 text-primary-foreground">
        <div className="container mx-auto space-y-6 px-4 text-center">
          <h2 className="text-3xl font-bold tracking-tight md:text-4xl">
            Built by GOTIGIN
          </h2>
          <p className="mx-auto max-w-2xl text-lg opacity-90 md:text-xl">
            Restaurant Operating System is the flagship product of GOTIGIN
            SOFTWARE & HARDWARE PRIVATE LIMITED.
          </p>
          <div className="pt-4">
            <a
              href="https://gotigin.com"
              target="_blank"
              rel="noopener noreferrer"
              className={cn(
                buttonVariants({ size: "lg" }),
                "h-12 bg-white px-8 text-lg text-primary hover:bg-[#f1eeec]"
              )}
            >
              Visit gotigin.com
            </a>
          </div>
        </div>
      </section>

      <Footer />
    </div>
  );
}
