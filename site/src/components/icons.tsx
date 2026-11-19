// The site's icon set: Hugeicons (same set as the web app). Every icon on the site comes from
// here — React islands import the components; Astro files render them too (server-only, no JS).
// Names describe the role, so swapping a glyph means changing one line.
import type { ComponentProps } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowDown01Icon,
  ArrowLeft01Icon,
  ArrowLeft02Icon,
  ArrowRight01Icon,
  ArrowUp01Icon,
  ArrowUpRight01Icon,
  Cancel01Icon,
  Coffee02Icon,
  Download04Icon,
  File02Icon,
  Globe02Icon,
  JusticeScale01Icon,
  Menu01Icon,
  MoreHorizontalIcon as MoreHorizontal,
  Search01Icon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";

type IconProps = Omit<ComponentProps<typeof HugeiconsIcon>, "icon">;

const make = (icon: IconSvgElement, name: string) => {
  const Icon = (props: IconProps) => <HugeiconsIcon icon={icon} aria-hidden="true" {...props} />;
  Icon.displayName = name;
  return Icon;
};

export const ArrowLeftIcon = make(ArrowLeft02Icon, "ArrowLeftIcon");
export const ArrowUpRightIcon = make(ArrowUpRight01Icon, "ArrowUpRightIcon");
export const CheckIcon = make(Tick02Icon, "CheckIcon");
export const ChevronDownIcon = make(ArrowDown01Icon, "ChevronDownIcon");
export const ChevronLeftIcon = make(ArrowLeft01Icon, "ChevronLeftIcon");
export const ChevronRightIcon = make(ArrowRight01Icon, "ChevronRightIcon");
export const ChevronUpIcon = make(ArrowUp01Icon, "ChevronUpIcon");
export const CoffeeIcon = make(Coffee02Icon, "CoffeeIcon");
export const DownloadIcon = make(Download04Icon, "DownloadIcon");
export const FileTextIcon = make(File02Icon, "FileTextIcon");
export const GlobeIcon = make(Globe02Icon, "GlobeIcon");
export const MenuIcon = make(Menu01Icon, "MenuIcon");
export const MoreHorizontalIcon = make(MoreHorizontal, "MoreHorizontalIcon");
export const ScaleIcon = make(JusticeScale01Icon, "ScaleIcon");
export const SearchIcon = make(Search01Icon, "SearchIcon");
export const XIcon = make(Cancel01Icon, "XIcon");
