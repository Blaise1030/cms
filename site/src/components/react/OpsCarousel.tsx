import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { paintingUrl, type PaintingId } from "@/lib/paintings";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@/components/ui/carousel";
import { StatusBadge } from "./StatusBadge";

const slides = [
  {
    title: "Collections you can browse",
    caption: "Every Markdown collection in the repo, as a list with status.",
    kind: "content" as const,
    painting: "altdorfer" as PaintingId,
  },
  {
    title: "Desktop and mobile preview",
    caption: "Share a draft link, leave comments, approve before Publish.",
    kind: "preview" as const,
    painting: "giorgione" as PaintingId,
  },
  {
    title: "Publish with a checklist",
    caption: "Build, links and review run first. One button puts it live.",
    kind: "publish" as const,
    painting: "bruegel" as PaintingId,
  },
  {
    title: "Ask in plain English",
    caption: "Whole pages, a pass on the FAQ, or a new section — as a draft.",
    kind: "agent" as const,
    painting: "leonardo" as PaintingId,
  },
  {
    title: "Checkpoints you can restore",
    caption: "Every save is history. Restoring makes a new draft, never a rewrite.",
    kind: "history" as const,
    painting: "patinir" as PaintingId,
  },
];

function SlideBody({ kind }: { kind: (typeof slides)[number]["kind"] }) {
  if (kind === "content") {
    return (
      <div className="space-y-2 rounded-md border border-line bg-surface-raised p-3">
        {["Welcome to Orso", "Release notes people read", "Plan a launch in a week"].map((title, i) => (
          <div key={title} className="flex items-center justify-between gap-3 border-b border-line py-2 last:border-0">
            <span className="truncate text-sm text-ink">{title}</span>
            <StatusBadge tone={i === 0 ? "draft" : i === 2 ? "review" : "live"} label={i === 0 ? "Draft" : i === 2 ? "Review" : "Live"} />
          </div>
        ))}
      </div>
    );
  }
  if (kind === "preview") {
    return (
      <div className="rounded-md border border-line bg-surface-raised p-4">
        <div className="h-2 w-24 rounded-full bg-ink" />
        <div className="mt-3 h-2 w-40 rounded-full bg-line" />
        <div className="mt-2 h-2 w-32 rounded-full bg-line" />
        <div className="mt-5 grid grid-cols-3 gap-2">
          <span className="h-14 rounded bg-sand" />
          <span className="h-14 rounded bg-sand" />
          <span className="h-14 rounded bg-sand" />
        </div>
      </div>
    );
  }
  if (kind === "publish") {
    return (
      <div className="rounded-md border border-line bg-surface-raised p-4">
        <p className="text-sm font-medium text-ink">Publish 3 changes</p>
        <ul className="mt-3 space-y-2 text-xs text-muted">
          <li className="flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-status-live" />
            Build passed
          </li>
          <li className="flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-status-live" />
            Links checked
          </li>
          <li className="flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-status-live" />
            Preview approved
          </li>
        </ul>
        <Badge className="mt-4">Publish to main</Badge>
      </div>
    );
  }
  if (kind === "agent") {
    return (
      <div className="space-y-3">
        <div className="ml-auto max-w-[85%] rounded-md bg-surface-raised px-3 py-2 text-xs text-ink">
          Tighten the pricing FAQ and add a refund line.
        </div>
        <div className="max-w-[85%] rounded-md border border-line bg-surface-raised px-3 py-2 text-xs text-muted">
          Draft ready. Open the preview when you want to check it.
        </div>
      </div>
    );
  }
  return (
    <div className="rounded-md border border-line bg-surface-raised p-3">
      {["Published to main", "Shortened the pricing FAQ", "New cover image"].map((title, i) => (
        <div key={title} className="flex items-center gap-3 border-b border-line py-2.5 last:border-0">
          <span className={`size-2 rounded-full ${i === 0 ? "bg-status-live" : "bg-line-strong"}`} />
          <span className="text-sm text-ink">{title}</span>
        </div>
      ))}
    </div>
  );
}

function slideBadge(kind: (typeof slides)[number]["kind"]) {
  if (kind === "content") return <StatusBadge tone="neutral" label="12 posts" />;
  if (kind === "preview") return <StatusBadge tone="review" label="Preview" />;
  if (kind === "publish") return <StatusBadge tone="review" label="Ready" />;
  if (kind === "agent") return <StatusBadge tone="draft" label="Draft ready" />;
  return <StatusBadge tone="live" label="Live" />;
}

export function OpsCarousel() {
  return (
    <Carousel opts={{ align: "start", loop: false }} className="w-full">
      <div className="mb-6 flex items-center justify-end gap-2">
        <CarouselPrevious className="static translate-y-0" />
        <CarouselNext className="static translate-y-0" />
      </div>
      <div className="relative">
        <CarouselContent className="-ml-5 items-stretch">
          {slides.map((slide) => (
            <CarouselItem key={slide.kind} className="basis-4/5 py-1 pl-5 md:basis-2/3">
              <Card className="flex h-full flex-col rounded-md border border-line shadow-soft">
                <CardHeader className="flex flex-row items-center gap-2 border-b border-line py-3">
                  <span className="font-mono text-xs text-muted">orso</span>
                  <span className="text-line-strong" aria-hidden="true">
                    /
                  </span>
                  <span className="text-xs text-muted">{slide.kind}</span>
                  <span className="ml-auto">{slideBadge(slide.kind)}</span>
                </CardHeader>
                <CardContent className="relative aspect-3/2 overflow-hidden p-0" aria-hidden="true">
                  <img
                    src={paintingUrl(slide.painting)}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    className="absolute inset-0 size-full object-cover"
                  />
                  <div className="absolute inset-0 bg-night/20" />
                  <div className="relative flex size-full items-start overflow-hidden p-6">
                    <div className="w-full">
                      <SlideBody kind={slide.kind} />
                    </div>
                  </div>
                </CardContent>
                <CardHeader className="mt-auto min-h-28 border-t border-line">
                  <CardTitle className="text-base text-ink">{slide.title}</CardTitle>
                  <CardDescription className="text-muted">{slide.caption}</CardDescription>
                </CardHeader>
              </Card>
            </CarouselItem>
          ))}
        </CarouselContent>
        <div
          className="pointer-events-none absolute inset-y-0 right-0 w-16 bg-linear-to-l from-surface to-transparent md:w-24"
          aria-hidden="true"
        />
      </div>
    </Carousel>
  );
}
