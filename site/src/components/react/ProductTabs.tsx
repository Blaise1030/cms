import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { paintingUrl } from "@/lib/paintings";
import { StatusBadge } from "./StatusBadge";

const tabs = [
  { value: "content", title: "Content", description: "Every collection in the repo, as a list you can use." },
  { value: "preview", title: "Preview", description: "See the draft on desktop and mobile before anyone else." },
  { value: "publish", title: "Publish", description: "Checks run first, then one button puts it live." },
  { value: "agent", title: "Agent", description: "Ask for a page, a pass or a whole new section." },
  { value: "history", title: "History", description: "Every save is a checkpoint you can go back to." },
];

const entries = [
  { title: "Welcome to Orso", tone: "draft" as const, label: "Draft", updated: "2 min ago", author: "Blaise" },
  { title: "Release notes people read", tone: "live" as const, label: "Published", updated: "Tuesday", author: "Ana" },
  { title: "How we run design reviews", tone: "live" as const, label: "Published", updated: "Monday", author: "Sam" },
  { title: "Plan a launch in a week", tone: "review" as const, label: "In review", updated: "Sunday", author: "Blaise" },
  { title: "Invite your team", tone: "live" as const, label: "Published", updated: "Friday", author: "Ana" },
];

const checks = [
  "Build passed in 42 seconds",
  "24 links checked, none broken",
  "Preview approved by Ana",
  "No conflicts with main",
];

const history = [
  { title: "Published to main", detail: "Blaise · just now", live: true },
  { title: "Shortened the pricing FAQ", detail: "Blaise · 14:20 · a1f9c2e", live: false },
  { title: "New cover image", detail: "Ana · 11:02 · 4c07b81", live: false },
  { title: "Restored from 9 November", detail: "Blaise · Monday · 8e2d114", live: false },
];

function WindowChrome({ crumbs, action }: { crumbs: string[]; action: ReactNode }) {
  return (
    <div className="flex min-h-11 shrink-0 items-center gap-2 border-b border-line px-4">
      <span className="font-mono text-xs tracking-tight text-muted">orso</span>
      {crumbs.map((crumb) => (
        <span key={crumb} className="flex items-center gap-2">
          <span className="text-line-strong" aria-hidden="true">
            /
          </span>
          <span className="truncate text-xs text-muted">{crumb}</span>
        </span>
      ))}
      <div className="ml-auto flex items-center gap-2">{action}</div>
    </div>
  );
}

function PaintedCard({ children }: { children: ReactNode }) {
  return (
    <div className="relative overflow-hidden">
      <img
        src={paintingUrl("patinir")}
        alt=""
        aria-hidden="true"
        loading="eager"
        decoding="async"
        className="absolute inset-0 size-full object-cover"
      />
      <div className="absolute inset-0 bg-night/25" aria-hidden="true" />
      <div className="relative p-4 md:p-6">
        <Card className="overflow-hidden rounded-md p-0 ring-line">{children}</Card>
      </div>
    </div>
  );
}

export function ProductTabs() {
  return (
    <Tabs
      defaultValue="content"
      className="gap-0 overflow-hidden rounded-lg bg-surface shadow-raised ring-1 ring-line"
    >
      <TabsList
        variant="line"
        className="grid h-auto! w-full grid-cols-2 items-stretch rounded-none border-b border-line bg-transparent p-0 md:grid-cols-5"
      >
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.value}
            value={tab.value}
            className="h-auto flex-col items-start gap-1 rounded-none border-r border-line px-4 py-4 text-left last:border-r-0 data-active:shadow-none md:px-6 md:py-5"
          >
            <span className="text-base font-medium text-ink">{tab.title}</span>
            <span className="text-sm leading-snug font-normal whitespace-normal text-muted">{tab.description}</span>
          </TabsTrigger>
        ))}
      </TabsList>

      <TabsContent value="content" className="mt-0">
        <PaintedCard>
          <WindowChrome crumbs={["site", "content", "blog"]} action={<StatusBadge tone="neutral" label="All content" />} />
          <div className="grid md:grid-cols-[12.5rem_minmax(0,1fr)]" aria-hidden="true">
            <aside className="hidden border-r border-line p-3 md:block">
              <p className="px-2 text-xs text-muted">cms / site</p>
              <ul className="mt-3 space-y-0.5 text-sm">
                {["Content", "Pages", "Media", "History"].map((item, i) => (
                  <li key={item} className={`rounded px-2 py-1.5 ${i === 0 ? "bg-sand font-medium text-ink" : "text-muted"}`}>
                    {item}
                  </li>
                ))}
              </ul>
            </aside>
            <div className="min-w-0 p-5 md:p-7">
              <div className="flex flex-wrap items-end justify-between gap-4">
                <div>
                  <h3 className="text-xl font-medium tracking-tight text-ink">Blog posts</h3>
                  <p className="mt-1 text-xs text-muted">5 of 12 shown · sorted by last saved</p>
                </div>
                <Button size="sm">New post</Button>
              </div>
              <div className="mt-5 flex flex-wrap gap-1.5">
                <Badge>All 12</Badge>
                <Badge variant="outline">Drafts 3</Badge>
                <Badge variant="outline">In review 2</Badge>
                <Badge variant="outline">Published 7</Badge>
              </div>
              <div className="mt-5">
                <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-4 border-b border-line pb-2 text-xs text-muted sm:grid-cols-[minmax(0,1fr)_7rem_auto]">
                  <span>Title</span>
                  <span className="hidden sm:block">Updated</span>
                  <span className="text-right">Status</span>
                </div>
                <ul>
                  {entries.map((entry) => (
                    <li
                      key={entry.title}
                      className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-line py-3 sm:grid-cols-[minmax(0,1fr)_7rem_auto]"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium text-ink">{entry.title}</p>
                        <p className="text-xs text-muted">
                          {entry.author} · content/blog/en
                        </p>
                      </div>
                      <span className="hidden text-xs text-muted sm:block">{entry.updated}</span>
                      <StatusBadge tone={entry.tone} label={entry.label} />
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          </div>
        </PaintedCard>
      </TabsContent>

      <TabsContent value="preview" className="mt-0">
        <PaintedCard>
          <WindowChrome
            crumbs={["site", "preview", "home"]}
            action={<StatusBadge tone="review" label="Preview" />}
          />
          <div className="grid md:grid-cols-[minmax(0,1fr)_17rem]" aria-hidden="true">
            <div className="bg-sand p-4">
              <div className="overflow-hidden rounded-md border border-line bg-surface-raised">
                <div className="flex items-center justify-between border-b border-line px-4 py-2.5">
                  <span className="text-xs font-medium text-ink">Orso</span>
                  <Button size="xs">Get Orso</Button>
                </div>
                <div className="p-5 md:p-7">
                  <p className="max-w-md text-xl font-medium leading-tight tracking-tight text-ink md:text-2xl">
                    Let your content team edit the site with an agent.
                  </p>
                  <p className="mt-3 max-w-sm text-xs leading-5 text-muted">
                    Drafts, previews and a Publish button on the repo your developer already set up.
                  </p>
                </div>
              </div>
            </div>
            <aside className="hidden border-l border-line p-4 md:block">
              <p className="text-xs font-medium text-ink">Comments</p>
              <div className="mt-3 rounded-md border border-line bg-surface p-3">
                <p className="text-xs font-medium text-ink">Ana</p>
                <p className="mt-1 text-xs leading-5 text-muted">Can we make this line shorter?</p>
              </div>
              <div className="mt-4 flex items-center justify-between border-t border-line pt-3">
                <span className="text-xs text-muted">2 comments</span>
                <Button size="xs">Approve</Button>
              </div>
            </aside>
          </div>
        </PaintedCard>
      </TabsContent>

      <TabsContent value="publish" className="mt-0">
        <PaintedCard>
          <WindowChrome crumbs={["site", "publish"]} action={<StatusBadge tone="review" label="Ready to publish" />} />
          <div className="grid place-items-center p-6" aria-hidden="true">
            <div className="w-full max-w-lg rounded-md border border-line bg-surface-raised p-6">
              <div className="flex items-center gap-2 text-xs text-muted">
                <span className="font-mono">draft/pricing-faq</span>
                <span>→</span>
                <span className="font-mono">main</span>
              </div>
              <h3 className="mt-3 text-xl font-medium tracking-tight text-ink">Publish 3 changes</h3>
              <p className="mt-1 text-sm text-muted">This saves a checkpoint, merges the draft and lets your host rebuild.</p>
              <Separator className="my-4" />
              <ul className="space-y-2.5">
                {checks.map((check) => (
                  <li key={check} className="flex items-center gap-2.5 text-sm text-ink">
                    <span className="size-1.5 rounded-full bg-status-live" />
                    {check}
                  </li>
                ))}
              </ul>
              <div className="mt-5 flex flex-wrap gap-2">
                <Button>Publish to main</Button>
                <Button variant="outline">Keep as draft</Button>
              </div>
            </div>
          </div>
        </PaintedCard>
      </TabsContent>

      <TabsContent value="agent" className="mt-0">
        <PaintedCard>
          <WindowChrome crumbs={["site", "agent"]} action={<StatusBadge tone="draft" label="Draft ready" />} />
          <div className="grid md:grid-cols-[minmax(0,1fr)_16rem]" aria-hidden="true">
            <div className="flex min-w-0 flex-col p-4 md:p-6">
              <div className="flex items-center gap-2 border-b border-line pb-3">
                <span className="text-xs font-medium text-ink">Agent</span>
                <Badge variant="outline">Claude</Badge>
                <Badge variant="outline">Cursor</Badge>
                <Badge variant="outline">Codex</Badge>
              </div>
              <div className="space-y-4 py-4">
                <div className="ml-auto max-w-md rounded-md bg-sand px-3 py-2.5 text-sm text-ink">
                  Build a landing page for group bookings with our usual hero and FAQ.
                </div>
                <div className="max-w-md">
                  <p className="text-xs font-medium text-ink">Orso</p>
                  <p className="mt-1 text-sm leading-6 text-muted">
                    Done. I reused the hero, pricing table and FAQ from the site, and left the copy as placeholders for
                    review.
                  </p>
                  <div className="mt-3 flex gap-2">
                    <Button size="sm">Open preview</Button>
                    <Button size="sm" variant="outline">
                      View changes
                    </Button>
                  </div>
                </div>
              </div>
            </div>
            <aside className="hidden border-l border-line p-4 md:block">
              <p className="text-xs font-medium text-ink">This draft</p>
              <p className="mt-4 text-xs text-muted">3 files · +46 −2</p>
              <p className="mt-4 border-t border-line pt-3 text-xs leading-5 text-muted">
                Use the agent your team already pays for. Orso only needs the repo.
              </p>
            </aside>
          </div>
        </PaintedCard>
      </TabsContent>

      <TabsContent value="history" className="mt-0">
        <PaintedCard>
          <WindowChrome crumbs={["site", "history"]} action={<StatusBadge tone="live" label="Live" />} />
          <div className="p-5 md:p-7" aria-hidden="true">
            <div className="flex items-end justify-between gap-4">
              <div>
                <h3 className="text-xl font-medium tracking-tight text-ink">History</h3>
                <p className="mt-1 text-xs text-muted">Every save is a checkpoint. Nothing is lost.</p>
              </div>
              <Badge variant="outline">main</Badge>
            </div>
            <ol className="mt-6">
              {history.map((item) => (
                <li key={item.title} className="grid grid-cols-[auto_minmax(0,1fr)_auto] gap-3 border-t border-line py-4">
                  <span className={`mt-1.5 size-2 rounded-full ${item.live ? "bg-status-live" : "bg-line-strong"}`} />
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-ink">{item.title}</p>
                    <p className="mt-0.5 font-mono text-[0.6875rem] text-muted">{item.detail}</p>
                  </div>
                  <span className="text-xs text-muted">{item.live ? "Live" : "Restore"}</span>
                </li>
              ))}
            </ol>
          </div>
        </PaintedCard>
      </TabsContent>
    </Tabs>
  );
}
