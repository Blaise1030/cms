import { buttonVariants } from "../components/ui/button";
import { cn } from "./utils";

/**
 * Rounded filter pill shared by the blog category filters and the help topic links: the shadcn
 * outline button, rounded, with the current page filled.
 */
export const filterPill = cn(
  buttonVariants({ variant: "outline", size: "sm" }),
  "rounded-full px-3 aria-[current=page]:bg-secondary aria-[current=page]:text-foreground",
);
