<script setup lang="ts">
import { computed, inject, ref, watchEffect } from "vue";
import type { FieldDescriptor } from "@/core/types";
import ImageDialog, { IMAGE_REGISTRY, type PendingImage } from "./ImageDialog.vue";
import { REFERENCE_ENTRIES } from "./references";

defineOptions({ name: "FrontmatterForm" });
const props = withDefaults(defineProps<{
  descriptor: FieldDescriptor;
  modelValue: unknown;
  /** Field path from the entry root; also keys validation messages and test ids. */
  path?: (string | number)[];
  label?: string;
  errors?: Record<string, string>;
}>(), { path: () => [], errors: () => ({}) });
const emit = defineEmits<{
  "update:modelValue": [value: unknown];
  /** An image field got a new image; its parent object also fills the alt-text companion. */
  image: [value: { value: string; alt: string }];
}>();

const key = computed(() => props.path.join("."));
const id = computed(() => `field-${key.value}`);
const message = computed(() => props.errors[key.value]);
const optional = computed(() => props.descriptor.required === false);
const objectValue = computed(() => (props.modelValue && typeof props.modelValue === "object" && !Array.isArray(props.modelValue) ? props.modelValue as Record<string, unknown> : {}));
const listValue = computed(() => (Array.isArray(props.modelValue) ? props.modelValue : []));
const variant = computed(() => {
  const d = props.descriptor;
  if (d.type !== "union" || !d.discriminator) return null;
  const chosen = objectValue.value[d.discriminator];
  return (d.variants ?? []).find(v => v.properties?.[d.discriminator!]?.values?.[0] === chosen) ?? null;
});
const variantNames = computed(() => (props.descriptor.variants ?? []).map(v => String(v.properties?.[props.descriptor.discriminator!]?.values?.[0])));

function set(value: unknown) { emit("update:modelValue", value); }
function setText(event: Event) {
  const text = (event.target as HTMLInputElement | HTMLSelectElement).value;
  set(text === "" && optional.value ? undefined : text);
}
function setNumber(event: Event) {
  const text = (event.target as HTMLInputElement).value;
  set(text === "" ? undefined : Number(text));
}
function setChild(name: string, value: unknown) {
  const { [name]: _old, ...rest } = objectValue.value;
  set(value === undefined ? rest : { ...rest, [name]: value });
}
/** `coverImage` → `coverAlt`, or `<name>Alt`, when the object declares it. */
function companion(name: string, properties: Record<string, FieldDescriptor> | undefined): string | null {
  for (const key of [name.replace(/Image$/, "Alt"), `${name}Alt`]) if (key !== name && properties?.[key]) return key;
  return null;
}
/** One update for an image field and its alt companion, so neither overwrites the other. */
function setImage(name: string, image: { value: string; alt: string }, properties: Record<string, FieldDescriptor> | undefined) {
  const alt = companion(name, properties);
  set({ ...objectValue.value, [name]: image.value, ...(alt ? { [alt]: image.alt } : {}) });
}

// Image fields: the dialog's image stays pending in the draft (shown by its preview) until Save.
const registry = inject(IMAGE_REGISTRY, null);
const imageOpen = ref(false);
const pendingPreview = computed(() => typeof props.modelValue === "string" && props.modelValue.startsWith("blob:"));
function chooseImage(image: PendingImage) {
  registry?.add(image);
  set(image.previewUrl);
  emit("image", { value: image.previewUrl, alt: image.alt });
}

// Reference fields choose among the target collection's entries; without them, free text.
const references = inject(REFERENCE_ENTRIES, null);
const referenceIds = computed(() => props.descriptor.type === "reference" && props.descriptor.collection ? references?.entries(props.descriptor.collection) : undefined);
watchEffect(() => { if (props.descriptor.type === "reference" && props.descriptor.collection && references && referenceIds.value === undefined) references.load(props.descriptor.collection); });
/** A saved value that names no current entry stays selectable so it is visible, not silently changed. */
const missingReference = computed(() => typeof props.modelValue === "string" && props.modelValue !== "" && !!referenceIds.value && !referenceIds.value.includes(props.modelValue) ? props.modelValue : null);

function setItem(index: number, value: unknown) {
  set(listValue.value.map((item, i) => (i === index ? value : item)));
}
function empty(d: FieldDescriptor | undefined): unknown {
  if (!d) return "";
  switch (d.type) {
    case "boolean": return false;
    case "number": return 0;
    case "object": return Object.fromEntries(Object.entries(d.properties ?? {}).filter(([, p]) => p.required !== false).map(([n, p]) => [n, empty(p)]));
    case "array": return [];
    case "union": return d.variants?.[0] ? empty(d.variants[0]) : {};
    case "literal": return d.values?.[0];
    case "enum": return d.values?.[0];
    default: return "";
  }
}
function chooseVariant(event: Event) {
  const name = (event.target as HTMLSelectElement).value;
  const next = (props.descriptor.variants ?? []).find(v => String(v.properties?.[props.descriptor.discriminator!]?.values?.[0]) === name);
  set(next ? { ...(empty(next) as object), [props.descriptor.discriminator!]: name } : {});
}
const box = "w-full rounded-md border bg-background px-2 py-1 text-sm";
</script>

<template>
  <div class="mb-3" :data-testid="`field-wrap-${key}`">
    <label v-if="label && descriptor.type !== 'boolean'" :for="id" class="mb-1 block text-xs font-medium">{{ label }}<span v-if="!optional" class="text-destructive"> *</span></label>

    <template v-if="descriptor.type === 'unsupported'">
      <p class="text-xs text-muted-foreground" :data-testid="`${id}-readonly`">Read only: {{ descriptor.reason }}</p>
      <pre v-if="modelValue !== undefined" class="whitespace-pre-wrap text-xs">{{ JSON.stringify(modelValue, null, 2) }}</pre>
    </template>

    <template v-else-if="descriptor.type === 'literal'">
      <p :id="id" class="text-sm text-muted-foreground">{{ descriptor.values?.[0] }}</p>
    </template>

    <label v-else-if="descriptor.type === 'boolean'" class="flex items-center gap-2 text-sm">
      <input :id="id" :data-testid="id" type="checkbox" :checked="modelValue === true" @change="set(($event.target as HTMLInputElement).checked)" />
      {{ label }}
    </label>

    <input v-else-if="descriptor.type === 'number'" :id="id" :data-testid="id" type="number" :class="box" :value="modelValue as number | undefined" @input="setNumber" />

    <input v-else-if="descriptor.type === 'date'" :id="id" :data-testid="id" type="date" :class="box" :value="modelValue as string | undefined" @input="setText" />

    <select v-else-if="descriptor.type === 'enum'" :id="id" :data-testid="id" :class="box" :value="modelValue as string | undefined" @change="setText">
      <option v-if="optional" value="">Not set</option>
      <option v-for="v in descriptor.values" :key="String(v)" :value="v as string">{{ v }}</option>
    </select>

    <template v-else-if="descriptor.type === 'array'">
      <div v-for="(item, index) in listValue" :key="index" class="mb-2 flex items-start gap-2" :data-testid="`${id}-item`">
        <div class="min-w-0 flex-1">
          <FrontmatterForm :descriptor="descriptor.items!" :model-value="item" :path="[...path, index]" :errors="errors" @update:model-value="setItem(index, $event)" />
        </div>
        <button type="button" class="text-xs underline" :data-testid="`${id}-remove-${index}`" @click="set(listValue.filter((_, i) => i !== index))">Remove</button>
      </div>
      <button type="button" class="text-xs underline" :data-testid="`${id}-add`" @click="set([...listValue, empty(descriptor.items)])">Add item</button>
    </template>

    <fieldset v-else-if="descriptor.type === 'object'" class="rounded-md border p-2">
      <FrontmatterForm v-for="(child, name) in descriptor.properties" :key="name" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, descriptor.properties)" />
    </fieldset>

    <fieldset v-else-if="descriptor.type === 'union'" class="rounded-md border p-2">
      <select :id="id" :data-testid="id" :class="box" :value="String(objectValue[descriptor.discriminator!] ?? '')" @change="chooseVariant">
        <option value="" disabled>Choose {{ descriptor.discriminator }}</option>
        <option v-for="name in variantNames" :key="name" :value="name">{{ name }}</option>
      </select>
      <template v-if="variant">
        <template v-for="(child, name) in variant.properties" :key="name">
          <FrontmatterForm v-if="name !== descriptor.discriminator" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" class="mt-2" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, variant.properties)" />
        </template>
      </template>
    </fieldset>

    <select v-else-if="descriptor.type === 'reference' && referenceIds" :id="id" :data-testid="id" :class="box" :value="(modelValue as string | undefined) ?? ''" @change="setText">
      <option value="" :disabled="!optional">{{ optional ? "Not set" : `Choose a ${descriptor.collection} entry` }}</option>
      <option v-for="entryId in referenceIds" :key="entryId" :value="entryId">{{ entryId }}</option>
      <option v-if="missingReference" :value="missingReference">{{ missingReference }} (missing)</option>
    </select>
    <input v-else-if="descriptor.type === 'reference'" :id="id" :data-testid="id" type="text" :class="box" :placeholder="`${descriptor.collection} entry`" :value="modelValue as string | undefined" @input="setText" />
    <div v-else-if="descriptor.type === 'image'" class="flex items-center gap-2">
      <img v-if="pendingPreview" :src="modelValue as string" alt="" class="h-10 w-10 shrink-0 rounded border object-cover" :data-testid="`${id}-preview`" />
      <input :id="id" :data-testid="id" type="text" :class="box" placeholder="Image path" :readonly="pendingPreview" :value="pendingPreview ? 'New image, uploaded on save' : modelValue as string | undefined" @input="setText" />
      <button type="button" class="shrink-0 text-xs underline" :data-testid="`${id}-choose`" @click="imageOpen = true">Choose image</button>
      <ImageDialog v-model:open="imageOpen" @insert="chooseImage" />
    </div>

    <input v-else :id="id" :data-testid="id" type="text" :class="box" :value="modelValue as string | undefined" @input="setText" />

    <p v-if="message" class="mt-1 text-xs text-destructive" role="alert" :data-testid="`${id}-error`">{{ key }}: {{ message }}</p>
  </div>
</template>
