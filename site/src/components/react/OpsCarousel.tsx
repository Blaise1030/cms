import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { paintingUrl, type PaintingId } from "@/lib/paintings";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@/components/ui/carousel";
import { cn } from "cn";
import type { AppShotData } from "@/lib/appShotTypes";
import { AppShot } from "./AppShot";

export const opsSlides = [
  {
    title: "Collections you can browse",
    caption: "Every Markdown collection in the repo, as a list with status.",
    kind: "content" as const,
    painting: "danube" as PaintingId,
  },
  {
    title: "Desktop and mobile preview",
    caption: "Share a draft link, leave comments, approve before Publish.",
    kind: "preview" as const,
    painting: "bridge" as PaintingId,
  },
  {
    title: "Publish with a checklist",
    caption: "Build, links and review run first. One button puts it live.",
    kind: "publish" as const,
    painting: "toledo" as PaintingId,
  },
  {
    title: "Ask in plain English",
    caption: "Whole pages, a pass on the FAQ, or a new section — as a draft.",
    kind: "agent" as const,
    painting: "alps" as PaintingId,
  },
  {
    title: "Checkpoints you can restore",
    caption: "Every save is history. Restoring makes a new draft, never a rewrite.",
    kind: "history" as const,
    painting: "rocks" as PaintingId,
  },
];

const slides = opsSlides;
type SlideKind = (typeof opsSlides)[number]["kind"];

const alts: Record<SlideKind, string> = {
  content: "Orso editing a blog post: the body on the left, its fields (slug, title, date, tags) in a form on the right.",
  preview: "Orso previewing a draft of the site, with the agent panel open beside it.",
  publish: "Orso's changes menu listing the live site and open drafts.",
  agent: "Orso's agent panel beside the site preview.",
  history: "An Orso checkpoint opened from the history timeline.",
};

export function OpsCarousel({ shots }: { shots: Record<SlideKind, AppShotData> }) {
  const last = slides.length - 1;

  return (
    <Carousel opts={{ align: "start", loop: false }} className="w-full">
      <div className="page mb-6 flex items-center justify-start gap-2 md:justify-end">
        <CarouselPrevious size="icon-lg" className="static translate-y-0" />
        <CarouselNext size="icon-lg" className="static translate-y-0" />
      </div>
      <div className="relative border-t border-line">
        <CarouselContent className="ml-0 items-stretch">
          {slides.map((slide, index) => (
            <CarouselItem
              key={slide.kind}
              className={cn(
                "basis-4/5 border-r border-line pl-0 md:basis-1/2 lg:basis-2/5",
                index === last && "border-r-0",
              )}
            >
              <Card className="flex h-full flex-col gap-0 rounded-none bg-transparent p-2 shadow-none ring-0">
                <CardContent className="relative shrink-0 overflow-hidden rounded-md border border-line p-0">
                  <img
                    src={paintingUrl(slide.painting)}
                    alt=""
                    aria-hidden="true"
                    loading="lazy"
                    decoding="async"
                    className="absolute inset-0 size-full object-cover"
                  />
                  <div className="absolute inset-0 bg-night/20" aria-hidden="true" />
                  <div className="relative p-2 md:p-3">
                    <AppShot
                      shot={shots[slide.kind]}
                      alt={alts[slide.kind]}
                      sizes="(min-width: 64rem) 32rem, (min-width: 48rem) 50vw, 80vw"
                      className="aspect-3/2 w-full"
                    />
                  </div>
                </CardContent>
                <CardHeader className="shrink-0 gap-1 px-4 pt-5 pb-4">
                  <CardTitle className="text-body text-ink">{slide.title}</CardTitle>
                  <CardDescription className="text-muted">{slide.caption}</CardDescription>
                </CardHeader>
              </Card>
            </CarouselItem>
          ))}
        </CarouselContent>
      </div>
    </Carousel>
  );
}
