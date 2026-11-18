import { useEffect, useRef, useState, type ReactNode } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AppShotData } from "@/lib/appShotTypes";
import { paintingUrl } from "@/lib/paintings";
import { AppShot } from "./AppShot";

export const productTabs = [
  {
    value: "content",
    title: "Content",
    description: "Every collection in the repo, as a list you can use.",
    alt: "Orso's editor open on a blog post: the markdown body beside its title, slug, date and tags.",
  },
  {
    value: "preview",
    title: "Preview",
    description: "See the draft on desktop and mobile before anyone else.",
    alt: "Orso previewing the same blog post rendered on the draft site, with the agent panel open beside it.",
  },
  {
    value: "publish",
    title: "Publish",
    description: "Checks run first, then one button puts it live.",
    alt: "Orso's changes menu over the preview, listing the live site and open drafts, next to Review and Publish.",
  },
  {
    value: "agent",
    title: "Agent",
    description: "Ask for a page, a pass or a whole new section.",
    alt: "Orso's agent panel next to the site preview, ready for a request.",
  },
  {
    value: "history",
    title: "History",
    description: "Every save is a checkpoint you can go back to.",
    alt: "Orso's history: a timeline of checkpoints with author, commit and time.",
  },
] as const;

type TabValue = (typeof productTabs)[number]["value"];
const tabs = productTabs;

const SLIDE_MS = 6000;

function PaintedStage({ children }: { children: ReactNode }) {
  return (
    <div className="relative aspect-video overflow-hidden">
      <img
        src={paintingUrl("danube")}
        alt=""
        aria-hidden="true"
        loading="eager"
        decoding="async"
        className="absolute inset-0 size-full object-cover"
      />
      <div className="absolute inset-0 bg-night/25" aria-hidden="true" />
      <div className="relative flex size-full flex-col p-1.5 sm:p-4 md:p-6">{children}</div>
    </div>
  );
}

export function ProductTabs({ shots }: { shots: Record<TabValue, AppShotData> }) {
  const [value, setValue] = useState<string>(tabs[0].value);
  const [progress, setProgress] = useState(0);
  const [paused, setPaused] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);
  const progressRef = useRef(0);
  const activeIndex = tabs.findIndex((tab) => tab.value === value);

  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setReducedMotion(media.matches);
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, []);

  useEffect(() => {
    if (reducedMotion || paused) return;

    let frame = 0;
    let last = performance.now();

    const tick = (now: number) => {
      const next = Math.min(1, progressRef.current + (now - last) / SLIDE_MS);
      last = now;
      progressRef.current = next;
      setProgress(next);
      if (next >= 1) {
        progressRef.current = 0;
        setProgress(0);
        setValue((current) => {
          const index = tabs.findIndex((tab) => tab.value === current);
          return tabs[(index + 1) % tabs.length].value;
        });
        return;
      }
      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [value, paused, reducedMotion]);

  const onValueChange = (next: string) => {
    progressRef.current = 0;
    setProgress(0);
    setValue(next);
  };

  return (
    <Tabs
      value={value}
      onValueChange={onValueChange}
      className="gap-0 overflow-hidden rounded-none border border-line bg-surface shadow-none"
      onPointerEnter={() => setPaused(true)}
      onPointerLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setPaused(false);
        }
      }}
    >
      <TabsList
        variant="line"
        className="relative z-10 flex h-auto! w-full items-stretch justify-start gap-0 overflow-x-auto rounded-none border-b border-line bg-transparent p-0 lg:overflow-visible"
      >
        {tabs.map((tab, index) => {
          const fill = index < activeIndex ? 1 : index === activeIndex ? (reducedMotion ? 1 : progress) : 0;
          return (
            <TabsTrigger
              key={tab.value}
              value={tab.value}
              className="relative h-auto w-64 min-w-64 shrink-0 grow-0 flex-none flex-col items-start justify-start gap-1 rounded-none border-r border-line px-4 py-4 pb-5 text-left after:hidden last:border-r-0 focus-visible:ring-0 focus-visible:outline-none data-active:shadow-none lg:w-auto lg:min-w-0 lg:flex-1 lg:grow lg:px-6 lg:py-5 lg:pb-6"
            >
              <span className="text-body font-medium text-ink">{tab.title}</span>
              <span className="text-small leading-snug font-normal whitespace-normal text-muted">{tab.description}</span>
              <span className="absolute inset-x-0 bottom-0 z-10 h-0.5 bg-line" aria-hidden="true">
                <span className="block h-full origin-left bg-ink" style={{ width: `${fill * 100}%` }} />
              </span>
            </TabsTrigger>
          );
        })}
      </TabsList>
      <PaintedStage>
        {tabs.map((tab, index) => (
          <TabsContent
            key={tab.value}
            value={tab.value}
            forceMount
            className="mt-0 flex min-h-0 flex-1 flex-col items-center justify-center data-[state=inactive]:hidden"
          >
            <AppShot
              shot={shots[tab.value]}
              alt={tab.alt}
              sizes="(min-width: 80rem) 76rem, 100vw"
              eager={index === 0}
              play={tab.value === "preview"}
              className="h-full max-h-full w-auto max-w-full"
            />
          </TabsContent>
        ))}
      </PaintedStage>
    </Tabs>
  );
}
