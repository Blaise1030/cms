import * as React from "react";
import { ChevronDownIcon, DownloadIcon } from "lucide-react";
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

const { version, repo, downloads } = site.release;

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
  React.useEffect(() => setPrimary(detectDownload()), []);

  return (
    <div className={cn("inline-flex items-stretch", className)}>
      <a
        className={cn(buttonVariants({ size: "lg" }), "rounded-r-none")}
        href={assetUrl(primary.target)}
        rel="noopener noreferrer"
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
              <a href={assetUrl(d.target)} rel="noopener noreferrer" className="justify-between">
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
  );
}
