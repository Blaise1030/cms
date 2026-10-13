import { defineComponent, h, ref, type PropType } from "vue";
import type { ContentCollection, ContentEntrySummary } from "@/core/types";
import type { TreeAction, TreeTarget } from "@/modules/content/contentTree";

/**
 * Stands in for `ContentTree.vue`, which renders @pierre/trees in a shadow DOM that happy-dom cannot lay out.
 * Clicking a collection opens it: its menu actions (`content-<id>`) and entries (`content-entry`) appear.
 */
export default defineComponent({
  name: "ContentTreeStub",
  props: {
    collections: { type: Array as PropType<readonly ContentCollection[]>, required: true },
    entries: { type: Object as PropType<Readonly<Record<string, readonly ContentEntrySummary[]>>>, required: true },
    selected: { type: String as PropType<string | null>, default: null },
    actions: { type: Function as PropType<(target: TreeTarget) => TreeAction[]>, required: true },
  },
  emits: { open: (_collection: string, _id: string) => true },
  setup(props, { emit }) {
    const open = ref<string | null>(null);
    return () => h("div", { "data-testid": "content-tree" }, props.collections.flatMap(collection => [
      h("button", { "data-testid": "content-collection", onClick: () => { open.value = collection.name; } }, collection.name),
      ...(open.value !== collection.name && !props.selected?.startsWith(`${collection.name}/`) ? [] : [
        ...props.actions({ collection: collection.name }).flatMap(action => action === "separator" ? [] : [
          h("button", { "data-testid": `content-${action.id}`, disabled: action.disabled, onClick: action.run }, action.label),
          action.disabled && action.hint ? h("p", { "data-testid": `content-${action.id}-reason` }, action.hint) : null,
        ]),
        ...(props.entries[collection.name] ?? []).map(entry =>
          h("button", { "data-testid": "content-entry", onClick: () => emit("open", collection.name, entry.id) }, entry.id)),
      ]),
    ]));
  },
});
