import { QueryClient, VueQueryPlugin } from "@tanstack/vue-query";

/** A fresh QueryClient per mount, so no cached server state leaks between tests. */
export function queryPlugin(client = new QueryClient({ defaultOptions: { queries: { gcTime: Infinity } } })) {
  return { client, plugin: [VueQueryPlugin, { queryClient: client }] as [typeof VueQueryPlugin, { queryClient: QueryClient }] };
}
