import * as React from "react";
import { ChevronDownIcon, CoffeeIcon, DownloadIcon, XIcon } from "lucide-react";
import { Dialog } from "radix-ui";
import { cn } from "cn";
import { buttonVariants } from "../ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { site } from "../../site";

const { version, repo, coffeeUrl, downloads } = site.release;
type Download = (typeof downloads)[number];

const assetUrl = (target: string) => `${repo}/releases/download/${version}/orso-${version}-${target}.tar.gz`;

/** Picks the build for the visitor's OS after hydration; macOS Apple Silicon is the server-rendered default. */
function detectDownload() {
  if (typeof navigator === "undefined") return downloads[0];
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("linux") && !ua.includes("android")) {
    return downloads.find((d) => d.os === "linux") ?? downloads[0];
  }
  return downloads[0];
}

export function DownloadButton({ className }: { className?: string }) {
  const [primary, setPrimary] = React.useState(downloads[0]);
  const [downloaded, setDownloaded] = React.useState<Download | null>(null);
  React.useEffect(() => setPrimary(detectDownload()), []);

  // The link still navigates to the file; the popup opens alongside the download.
  const onDownload = (d: Download) => () => setDownloaded(d);

  return (
    <>
      <div className={cn("inline-flex items-stretch", className)}>
        <a
          className={cn(buttonVariants({ size: "lg" }), "rounded-r-none")}
          href={assetUrl(primary.target)}
          rel="noopener noreferrer"
          onClick={onDownload(primary)}
        >
          <DownloadIcon data-icon="inline-start" />
          Download for {primary.label}
        </a>
        <DropdownMenu>
          <DropdownMenuTrigger
            className={cn(buttonVariants({ size: "icon-lg" }), "rounded-l-none border-l border-l-primary-foreground/20")}
            aria-label="Choose another platform"
          >
            <ChevronDownIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            {downloads.map((d) => (
              <DropdownMenuItem key={d.target} asChild>
                <a
                  href={assetUrl(d.target)}
                  rel="noopener noreferrer"
                  className="justify-between"
                  onClick={onDownload(d)}
                >
                  <span className="text-ink">{d.label}</span>
                  <span className="text-muted">{d.detail} .tar.gz</span>
                </a>
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem asChild>
              <a href={`${repo}/releases/latest`} rel="noopener noreferrer" className="text-muted">
                All releases on GitHub
              </a>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <ThanksDialog download={downloaded} onClose={() => setDownloaded(null)} />
    </>
  );
}

function ThanksDialog({ download, onClose }: { download: Download | null; onClose: () => void }) {
  const isMac = download?.os === "mac";
  return (
    <Dialog.Root open={download !== null} onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/30 backdrop-blur-md data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
        <Dialog.Content className="fixed top-1/2 left-1/2 z-50 w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-line bg-surface p-6 shadow-lg data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95">
          <Dialog.Close
            className={cn(buttonVariants({ variant: "ghost", size: "icon-sm" }), "absolute top-3 right-3")}
            aria-label="Close"
          >
            <XIcon />
          </Dialog.Close>

          <Dialog.Title className="pr-8 text-body font-medium text-ink">Thanks for downloading Orso</Dialog.Title>
          <Dialog.Description asChild>
            <div className="mt-3 space-y-3 text-small text-muted">
              <p>Your download should start in a moment.</p>
              {isMac ? (
                <>
                  <p>
                    Orso isn't notarized by Apple yet, so macOS may block it the first time. After unpacking, run this
                    once to allow it:
                  </p>
                  <code className="block overflow-x-auto rounded-md border border-line bg-surface-raised px-3 py-2 whitespace-nowrap text-ink">
                    xattr -d com.apple.quarantine ./orso
                  </code>
                  <p>Notarizing costs $99 a year. If Orso is useful to you, a coffee helps cover it.</p>
                </>
              ) : (
                <p>Orso is free and built in my spare time. If it's useful to you, a coffee helps keep it going.</p>
              )}
            </div>
          </Dialog.Description>

          <div className="mt-6 flex flex-wrap gap-3">
            <a className={buttonVariants({ size: "lg" })} href={coffeeUrl} target="_blank" rel="noopener noreferrer">
              <CoffeeIcon data-icon="inline-start" />
              Buy me a coffee
            </a>
            <Dialog.Close className={buttonVariants({ variant: "outline", size: "lg" })}>Maybe later</Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
