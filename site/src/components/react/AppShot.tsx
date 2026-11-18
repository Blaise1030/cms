import { cn } from "cn";
import type { AppShotData } from "@/lib/appShotTypes";

interface Props {
  shot: AppShotData;
  alt: string;
  /** `sizes` for the srcset; defaults to the full viewport width. */
  sizes?: string;
  eager?: boolean;
  /** Play the recorded clip (when the shot has one); reduced motion keeps the still. */
  play?: boolean;
  className?: string;
}

/**
 * A real Orso app capture in the site's current colour scheme. The light/dark pair switches on
 * prefers-color-scheme, the same signal the design tokens follow.
 */
export function AppShot({ shot, alt, sizes = "100vw", eager = false, play = false, className }: Props) {
  const video = play ? shot.video : undefined;
  return (
    <div
      className={cn("relative overflow-hidden rounded-md border border-line bg-surface shadow-lg", className)}
      style={{ aspectRatio: `${shot.width} / ${shot.height}` }}
    >
      <picture>
        <source media="(prefers-color-scheme: dark)" srcSet={shot.dark.srcSet} sizes={sizes} />
        <img
          src={shot.light.src}
          srcSet={shot.light.srcSet}
          sizes={sizes}
          width={shot.width}
          height={shot.height}
          alt={alt}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          className="block size-full object-cover object-top-left"
        />
      </picture>
      {video &&
        (["light", "dark"] as const).map((scheme) => (
          <video
            key={scheme}
            src={video[scheme]}
            className={cn(
              "absolute inset-0 size-full object-cover object-top-left motion-reduce:hidden!",
              scheme === "light" ? "scheme-light" : "scheme-dark",
            )}
            autoPlay
            muted
            loop
            playsInline
            preload="metadata"
            aria-hidden="true"
          />
        ))}
    </div>
  );
}
