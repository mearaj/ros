"use client";

import { useState } from "react";
import { Download, FileKey, Package } from "lucide-react";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  platformAssets,
  productRelease,
  type PlatformId,
} from "@/lib/downloads";
import { cn } from "@/lib/utils";

const defaultPlatformId: PlatformId = "linux";

export function ProductDownload() {
  const [selectedPlatformId, setSelectedPlatformId] =
    useState<PlatformId>(defaultPlatformId);

  const platform = platformAssets.find((item) => item.id === selectedPlatformId);
  const canDownload = platform?.status === "available";

  return (
    <Card className="mx-auto max-w-2xl transition-shadow duration-300 hover:shadow-lg">
      <CardHeader className="text-center">
        <div className="mx-auto mb-4 flex size-14 items-center justify-center rounded-xl bg-primary/10 text-primary">
          <Package className="size-7" />
        </div>
        <CardTitle className="text-2xl">{productRelease.name}</CardTitle>
        <CardDescription className="text-base">
          {productRelease.description}
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-6">
        <p className="text-center text-sm text-muted-foreground">
          Version{" "}
          <span className="font-medium text-foreground">
            {productRelease.version}
          </span>
        </p>

        <div className="space-y-3">
          <p className="text-center text-sm font-medium">Select platform</p>
          <div className="flex flex-wrap justify-center gap-2">
            {platformAssets.map((item) => {
              const isSelected = item.id === selectedPlatformId;
              const isPlanned = item.status === "planned";

              return (
                <Button
                  key={item.id}
                  type="button"
                  variant={isSelected ? "default" : "outline"}
                  size="sm"
                  onClick={() => setSelectedPlatformId(item.id)}
                  className={cn(isPlanned && !isSelected && "opacity-70")}
                >
                  {item.name}
                  {isPlanned ? " · planned" : null}
                </Button>
              );
            })}
          </div>
        </div>

        {platform?.status === "planned" ? (
          <p className="text-center text-sm text-muted-foreground">
            <span className="font-medium text-foreground">{platform.name}</span>{" "}
            is planned for a future release.
          </p>
        ) : null}

        <p className="flex items-start gap-2 text-sm text-muted-foreground">
          <FileKey className="mt-0.5 size-4 shrink-0" />
          <span>
            A detached signature file is provided so you can verify the
            download before installing. Community, Professional, and
            Enterprise capabilities are unlocked during setup — not by choosing
            a separate installer.
          </span>
        </p>
      </CardContent>

      <CardFooter className="flex flex-col gap-2 sm:flex-row sm:justify-center">
        {canDownload && platform ? (
          <>
            <a
              href={platform.binaryUrl}
              download={platform.filename}
              className={cn(buttonVariants({ size: "lg" }), "w-full sm:w-auto")}
            >
              <Download className="size-4" />
              Download for {platform.name}
            </a>
            <a
              href={platform.signatureUrl}
              download={platform.signatureFilename}
              className={cn(
                buttonVariants({ size: "lg", variant: "outline" }),
                "w-full sm:w-auto"
              )}
            >
              <FileKey className="size-4" />
              Signature
            </a>
          </>
        ) : (
          <span
            className={cn(
              buttonVariants({ size: "lg", variant: "secondary" }),
              "pointer-events-none w-full opacity-60 sm:w-auto"
            )}
          >
            {platform?.name ?? "This platform"} — planned
          </span>
        )}
      </CardFooter>
    </Card>
  );
}
