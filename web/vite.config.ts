import path from "node:path";
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import ui from "@nuxt/ui/vite";
import { VitePWA } from "vite-plugin-pwa";
import { pwaOptions } from "./pwa.config.ts";

// `just desktop-dev` moves both so it can run next to `just dev`; the desktop shell reads
// ORSO_DAEMON_PORT too, to start its daemon where this proxies.
const daemonPort = process.env.ORSO_DAEMON_PORT ?? "7800";
const uiPort = Number(process.env.ORSO_VITE_PORT ?? 5173);

export default defineConfig({
  plugins: [
    vue(),
    // Nuxt UI registers Tailwind's Vite plugin itself, so `@tailwindcss/vite` is not added here.
    ui({
      // The app owns light/dark (`lib/theme.ts` + the inline script in index.html).
      colorMode: false,
      // Components are imported explicitly; nothing is auto-imported.
      autoImport: false,
      components: false,
      ui: {
        colors: { primary: "blue", neutral: "neutral" },
        // Splitter panels are flex rows; make each panel's content fill it, as the old shadcn
        // resizable panels did, so views that measure their own size (the preview fit) see the
        // real panel width instead of their content's.
        splitter: { slots: { panel: "flex *:min-w-0 *:flex-1" } },
        // Medusa-style depth on every pressable surface (utilities in index.css): solid buttons
        // get a hairline edge, a 1px drop and a lit top edge; framed and selected surfaces a
        // lighter version. Ghost buttons stay flat until hovered so toolbars stay quiet; menu and
        // palette rows stay flat (they highlight on hover instead).
        button: {
          // Secondary (neutral subtle) buttons are white like outline ones; grey `soft` stays for
          // selected states.
          compoundVariants: [
            {
              color: "neutral",
              variant: "subtle",
              class: "bg-default hover:bg-elevated active:bg-elevated disabled:bg-default aria-disabled:bg-default",
            },
          ],
          variants: {
            variant: {
              solid: "btn-depth",
              outline: "btn-depth-soft",
              subtle: "btn-depth-soft",
              soft: "btn-depth-soft",
              ghost: "hover:btn-depth-soft",
            },
          },
        },
        select: { variants: { variant: { outline: "btn-depth-soft", subtle: "btn-depth-soft" } } },
        // Text fields get a soft drop under their ring (`field-depth`, index.css).
        input: { variants: { variant: { outline: "field-depth", subtle: "field-depth" } } },
        textarea: { variants: { variant: { outline: "field-depth", subtle: "field-depth" } } },
        inputTags: { variants: { variant: { outline: "field-depth", subtle: "field-depth" } } },
        radioGroup: { variants: { variant: { card: { item: "btn-depth-soft" } } } },
        checkbox: { variants: { variant: { card: { root: "btn-depth-soft" } } } },
        // Neutral pill tabs: a white raised active tab on the grey track (Nuxt UI's default paints
        // it inverted/black). A compound variant, so it lands after the built-in neutral+pill one.
        tabs: {
          compoundVariants: [
            {
              color: "neutral",
              variant: "pill",
              class: {
                indicator: "bg-default btn-depth-soft ring ring-default",
                trigger: [
                  "data-[state=active]:text-highlighted outline-primary/25",
                  // Used when the list has no measured indicator: the active tab paints its own.
                  "in-[[data-slot=list]:not(:has([data-slot=indicator]))]:data-[state=active]:before:bg-default",
                ],
              },
            },
          ],
        },
        // The app's icon set is Hugeicons; swap Nuxt UI's built-in Lucide defaults to match.
        icons: {
          arrowDown: "i-hugeicons-arrow-down-02",
          arrowLeft: "i-hugeicons-arrow-left-02",
          arrowRight: "i-hugeicons-arrow-right-02",
          arrowUp: "i-hugeicons-arrow-up-02",
          caution: "i-hugeicons-alert-circle",
          check: "i-hugeicons-tick-02",
          chevronDoubleLeft: "i-hugeicons-arrow-left-double",
          chevronDoubleRight: "i-hugeicons-arrow-right-double",
          chevronDown: "i-hugeicons-arrow-down-01",
          chevronLeft: "i-hugeicons-arrow-left-01",
          chevronRight: "i-hugeicons-arrow-right-01",
          chevronUp: "i-hugeicons-arrow-up-01",
          close: "i-hugeicons-cancel-01",
          copy: "i-hugeicons-copy-01",
          copyCheck: "i-hugeicons-tick-double-02",
          dark: "i-hugeicons-moon-02",
          drag: "i-hugeicons-drag-drop-vertical",
          ellipsis: "i-hugeicons-more-horizontal",
          error: "i-hugeicons-cancel-circle",
          external: "i-hugeicons-arrow-up-right-01",
          eye: "i-hugeicons-view",
          eyeOff: "i-hugeicons-view-off",
          file: "i-hugeicons-file-01",
          folder: "i-hugeicons-folder-01",
          folderOpen: "i-hugeicons-folder-open",
          hash: "i-hugeicons-hashtag",
          info: "i-hugeicons-information-circle",
          light: "i-hugeicons-sun-03",
          loading: "i-hugeicons-loading-03",
          menu: "i-hugeicons-menu-01",
          minus: "i-hugeicons-minus-sign",
          panelClose: "i-hugeicons-sidebar-left-01",
          panelOpen: "i-hugeicons-sidebar-left",
          plus: "i-hugeicons-plus-sign",
          reload: "i-hugeicons-arrow-turn-backward",
          search: "i-hugeicons-search-01",
          stop: "i-hugeicons-stop",
          star: "i-hugeicons-star",
          success: "i-hugeicons-checkmark-circle-02",
          system: "i-hugeicons-computer",
          tip: "i-hugeicons-bulb",
          upload: "i-hugeicons-upload-01",
          warning: "i-hugeicons-alert-02",
        },
      },
      // The app runs offline (embedded in the daemon / desktop shell): bundle every Hugeicons icon
      // used in `icon="i-hugeicons-…"` props instead of fetching from the Iconify API.
      icon: { clientBundle: { scan: true } },
      // Only generate theme CSS for the Nuxt UI components the app uses (scanned at startup, so
      // restart `vite` after using a new component). Without it, every component's classes ship.
      experimental: { componentDetection: true },
    }),
    VitePWA(pwaOptions),
  ],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "packages/*/src/**/*.test.ts"],
    // Component tests opt in with `// @vitest-environment happy-dom`; never load iframe URLs.
    environmentOptions: { happyDOM: { settings: { disableIframePageLoading: true } } },
  },
  server: {
    // Fixed so the daemon's dev ready link (`just dev`, --ui-url) always points here.
    port: uiPort,
    strictPort: true,
    proxy: {
      // `ws: true` is required: the terminal lives at `/api/terminal/{id}/ws`.
      // The string form proxies HTTP only, so the panel stays blank in `just dev`.
      "/api": { target: `http://127.0.0.1:${daemonPort}`, ws: true },
      "/ws": { target: `ws://127.0.0.1:${daemonPort}`, ws: true },
      "/preview": `http://127.0.0.1:${daemonPort}`,
    },
  },
});
