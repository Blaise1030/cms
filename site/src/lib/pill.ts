import { badgeVariants } from "../components/ui/badge";
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

/** Blog category label on cards and posts: the shadcn outline badge, as a link. */
export const categoryBadge = cn(badgeVariants({ variant: "outline" }), "relative z-10");

/** Rounded outline "Back to …" link with a leading arrow icon. */
export const backButton = cn(buttonVariants({ variant: "outline", size: "sm" }), "rounded-full");

/** Larger filter pill, used where the filters sit beside the search bar (blog list). */
export const filterPillLg = cn(
  buttonVariants({ variant: "outline", size: "lg" }),
  "rounded-full px-4 aria-[current=page]:bg-secondary aria-[current=page]:text-foreground",
);
