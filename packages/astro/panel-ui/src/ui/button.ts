import type { VariantProps } from "class-variance-authority";
import { cva } from "class-variance-authority";

/** Mirrors `web/src/components/ui/button` variants for the injected panel. */
export const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-all active:translate-y-px disabled:pointer-events-none disabled:opacity-50 shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-3 border border-transparent",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground btn-raised [--raised-tint:var(--primary)] hover:bg-primary/90",
        destructive:
          "bg-destructive text-white btn-raised [--raised-tint:var(--destructive)] hover:bg-destructive/90",
        outline:
          "bg-background btn-raised-soft hover:bg-accent hover:text-accent-foreground dark:bg-input/30",
        secondary: "bg-secondary text-secondary-foreground btn-raised-soft hover:bg-secondary/80",
        ghost: "hover:bg-background hover:text-accent-foreground",
        link: "active:translate-y-0 text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2",
        xs: "h-6 gap-1 rounded-md px-2 text-xs",
        sm: "h-8 rounded-md gap-1.5 px-3",
        lg: "h-10 rounded-md px-6",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export type ButtonVariants = VariantProps<typeof buttonVariants>;
