import { defineComponent, h, type Component } from "vue";
import UIcon from "@nuxt/ui/components/Icon.vue";

/**
 * A component for one Hugeicons icon, for places that take an icon as a component (palette
 * items, entry actions) rather than an `icon="i-hugeicons-…"` prop.
 * Always pass the full literal name: Nuxt UI's client bundle scans sources for `i-hugeicons-…`
 * strings, so a computed name would not be bundled for offline use.
 */
export function hugeicon(name: `i-hugeicons-${string}`): Component {
  return defineComponent({
    name: "HugeIcon",
    inheritAttrs: true,
    setup: () => () => h(UIcon, { name }),
  });
}
