import Cta from "./Cta.astro";
import Faq from "./Faq.astro";
import Features from "./Features.astro";
import Hero from "./Hero.astro";
import Logos from "./Logos.astro";
import Pricing from "./Pricing.astro";
import Testimonials from "./Testimonials.astro";

/** Section `type` → component. Add a new entry when you add a schema variant. */
export const sectionComponents = {
  hero: Hero,
  features: Features,
  pricing: Pricing,
  testimonials: Testimonials,
  faq: Faq,
  cta: Cta,
  logos: Logos,
} as const;
