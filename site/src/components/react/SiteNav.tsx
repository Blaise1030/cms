import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { MenuIcon } from "lucide-react";

export type NavItem = {
  label: string;
  href: string;
  current?: boolean;
};

export function SiteNav({ items }: { items: NavItem[] }) {
  return (
    <>
      <nav className="hidden items-center gap-5 pe-4 lg:flex" aria-label="Main">
        {items.map((link) => (
          <a
            key={link.href}
            className="shrink-0 py-1 text-sm text-muted transition-colors duration-quick ease-gentle hover:text-ink aria-[current=page]:text-ink"
            href={link.href}
            aria-current={link.current ? "page" : undefined}
          >
            {link.label}
          </a>
        ))}
      </nav>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="lg" className="gap-2 lg:hidden" aria-label="Open menu">
            <MenuIcon className="size-4" />
            <span className="hidden sm:inline">Menu</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-48 border border-line shadow-none ring-0">
          {items.map((link) => (
            <DropdownMenuItem key={link.href} asChild>
              <a href={link.href} aria-current={link.current ? "page" : undefined} className="cursor-pointer">
                {link.label}
              </a>
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </>
  );
}
