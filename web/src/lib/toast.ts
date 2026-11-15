// App toasts, shown by Nuxt UI's toaster (`UApp`) with its default look.
//
// Stores and components call `toast.success(…)` etc. from anywhere, including async code that runs
// outside a component's setup. Nuxt UI's `useToast()` must be called during setup, so `App.vue`
// calls it once and hands it over with `installToast`. Before that (or in unit tests that do not
// mount `App.vue`) toasts are dropped.
import type { useToast } from "@nuxt/ui/composables/useToast";

type ToastApi = ReturnType<typeof useToast>;
type Color = "success" | "error" | "info" | "warning" | "neutral";

export interface ToastOptions {
  /** Reusing an id replaces that toast in place (e.g. a loading toast that later succeeds). */
  id?: string | number;
  description?: string;
  /** Milliseconds; `0` keeps the toast until it is replaced or closed. */
  duration?: number;
}

let api: ToastApi | null = null;

/** Called once from `App.vue` setup with Nuxt UI's toast API. */
export function installToast(next: ToastApi) {
  api = next;
}

const ICONS: Record<Color, string> = {
  success: "i-hugeicons-checkmark-circle-02",
  error: "i-hugeicons-cancel-circle",
  info: "i-hugeicons-information-circle",
  warning: "i-hugeicons-alert-02",
  neutral: "i-hugeicons-loading-03",
};

function show(color: Color, title: string, options: ToastOptions = {}, extra: Record<string, unknown> = {}) {
  if (!api) return undefined;
  const { id, ...rest } = options;
  const toast = { title, color, icon: ICONS[color], ...rest, ...extra };
  if (id != null && api.toasts.value.some((t) => t.id === id)) {
    api.update(id, toast);
    return id;
  }
  return api.add({ ...(id != null ? { id } : {}), ...toast }).id;
}

export const toast = {
  success: (title: string, options?: ToastOptions) => show("success", title, options),
  error: (title: string, options?: ToastOptions) => show("error", title, options),
  info: (title: string, options?: ToastOptions) => show("info", title, options),
  warning: (title: string, options?: ToastOptions) => show("warning", title, options),
  /** A spinner toast that stays until it is replaced by another toast with the same id. */
  loading: (title: string, options?: ToastOptions) =>
    show("neutral", title, { duration: 0, ...options }, { ui: { icon: "animate-spin" } }),
};
