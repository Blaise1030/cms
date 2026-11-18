import { Button } from "@/components/ui/button";
import { MenuIcon, XIcon } from "lucide-react";
import { Dialog } from "radix-ui";

export type NavItem = {
  label: string;
  href: string;
  current?: boolean;
};

/** Desktop link row — place before the header CTAs. */
export function SiteNavLinks({ items }: { items: NavItem[] }) {
  return (
    <nav className="hidden items-center gap-5 pe-3 lg:flex" aria-label="Main">
      {items.map((link) => (
        <a
          key={link.href}
          className="shrink-0 py-1 text-small text-muted transition-colors duration-quick ease-gentle hover:text-ink aria-[current=page]:text-ink"
          href={link.href}
          aria-current={link.current ? "page" : undefined}
        >
          {link.label}
        </a>
      ))}
    </nav>
  );
}

/** Tablet/mobile menu — place after the header CTAs. Opens a full-height inset sheet. */
export function SiteNavMenu({ items }: { items: NavItem[] }) {
  return (
    <Dialog.Root>
      <Dialog.Trigger asChild>
        <Button variant="outline" size="icon-lg" className="lg:hidden" aria-label="Open menu">
          <MenuIcon className="size-4" />
        </Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-night/60 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0 lg:hidden" />
        <Dialog.Content
          aria-describedby={undefined}
          className="fixed inset-2 z-50 flex flex-col overflow-y-auto rounded-xl border border-line bg-surface-raised text-ink outline-none data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 lg:hidden"
        >
          <Dialog.Title className="sr-only">Menu</Dialog.Title>
          <nav className="my-auto px-6 py-24" aria-label="Main">
            <p className="text-label tracking-wide text-muted uppercase" aria-hidden="true">
              Menu
            </p>
            <ul className="mt-3 space-y-1">
              {items.map((link) => (
                <li key={link.href}>
                  <Dialog.Close asChild>
                    <a
                      href={link.href}
                      aria-current={link.current ? "page" : undefined}
                      className="block py-1.5 text-heading font-medium tracking-tight text-ink transition-colors duration-quick ease-gentle hover:text-muted aria-[current=page]:text-muted"
                    >
                      {link.label}
                    </a>
                  </Dialog.Close>
                </li>
              ))}
            </ul>
          </nav>
          <Dialog.Close asChild>
            <button
              type="button"
              className="sticky bottom-6 mx-auto mb-6 inline-flex shrink-0 items-center gap-2 rounded-full bg-ink px-5 py-2.5 text-small font-medium text-surface shadow-lg transition-opacity hover:opacity-90 focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:outline-none"
            >
              <XIcon className="size-4" aria-hidden="true" />
              Close
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
