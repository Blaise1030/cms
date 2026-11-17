import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const tones = {
  draft: "border-transparent bg-status-draft/10 text-status-draft",
  review: "border-transparent bg-status-review/10 text-status-review",
  live: "border-transparent bg-status-live/10 text-status-live",
  neutral: "border-line bg-sand text-muted",
} as const;

export function StatusBadge({
  tone,
  label,
  className,
}: {
  tone: keyof typeof tones;
  label: string;
  className?: string;
}) {
  return (
    <Badge variant="outline" className={cn("gap-1.5 rounded-full", tones[tone], className)}>
      <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
      {label}
    </Badge>
  );
}
