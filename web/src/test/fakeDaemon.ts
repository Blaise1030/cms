// Test-only stand-in for `useDaemon()`: API methods are vi.fn stubs supplied by the test,
// and `emit()` pushes a wire event to whatever the store subscribed.
import { ref } from "vue";
import type { DaemonEvent, DaemonEventType } from "@/core/types";

export function createFakeDaemon(api: Record<string, unknown> = {}) {
  const handlers = new Map<string, Set<(e: DaemonEvent) => void>>();
  const reconnect = new Set<() => void>();
  return {
    api: {
      previewUrl: (id: string) => `http://127.0.0.1:7800/preview/${id}/`,
      attachmentUrl: (project: string, name: string) =>
        `http://127.0.0.1:7800/api/chat/${project}/attachments/${name}`,
      ...api,
    },
    config: ref({ origin: "http://127.0.0.1:7800" }),
    status: ref("open"),
    on(type: DaemonEventType, handler: (e: never) => void) {
      let set = handlers.get(type);
      if (!set) handlers.set(type, (set = new Set()));
      set.add(handler as (e: DaemonEvent) => void);
      return () => set.delete(handler as (e: DaemonEvent) => void);
    },
    onReconnected(handler: () => void) {
      reconnect.add(handler);
      return () => reconnect.delete(handler);
    },
    emit(event: DaemonEvent) {
      for (const h of handlers.get(event.type) ?? []) h(event);
    },
    simulateReconnect() {
      for (const h of reconnect) h();
    },
    subscriberCount: () => [...handlers.values()].reduce((n, s) => n + s.size, 0),
  };
}

export type FakeDaemon = ReturnType<typeof createFakeDaemon>;
