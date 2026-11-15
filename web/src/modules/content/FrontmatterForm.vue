<script setup lang="ts">
import { computed, defineAsyncComponent, defineComponent, inject, ref, watchEffect } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCheckbox from "@nuxt/ui/components/Checkbox.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import UInputTags from "@nuxt/ui/components/InputTags.vue";
import USelect from "@nuxt/ui/components/Select.vue";
import { useFormField } from "@nuxt/ui/composables/useFormField";
// Kept: UFileUpload holds Files, not the saved image path this field shows.
import { ImageUploader } from "@/components/ui/image-uploader";
import type { FieldDescriptor } from "@/core/types";
import { humanize, humanizeLower } from "@orso/utils";
// The calendar is only needed for date fields, so it stays out of the startup bundle.
const DatePicker = defineAsyncComponent(() => import("./DatePicker.vue"));
import ImageDialog, { ACCEPTED_IMAGES, IMAGE_REGISTRY, type PendingImage } from "./ImageDialog.vue";
import { REFERENCE_ENTRIES } from "./references";

defineOptions({ name: "FrontmatterForm" });

/** Points the surrounding UFormField's label at a control that is not a Nuxt UI input. */
const LabelTarget = defineComponent({
  props: { id: { type: String, required: true } },
  setup(p, { slots }) { useFormField(p); return () => slots.default?.(); },
});
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
const variantItems = computed<Item[]>(() => variantNames.value.map(name => ({ label: name, value: name })));

function set(value: unknown) { emit("update:modelValue", value); }
function setText(value: unknown) {
  const text = String(value ?? "");
  set(text === "" && optional.value ? undefined : text);
}
function setNumber(value: unknown) {
  set(value === "" || value === undefined ? undefined : Number(value));
}
/** Select items cannot hold an empty value, so "Not set" uses this one. */
const UNSET = "__unset__";
function setChoice(value: unknown) { setText(value === UNSET ? "" : value); }
const choice = (value: unknown) => (value === undefined || value === null || value === "" ? undefined : String(value));
type Item = { label: string; value: string };
const notSet = computed<Item[]>(() => (optional.value ? [{ label: "Not set", value: UNSET }] : []));
const enumItems = computed<Item[]>(() => [...notSet.value, ...(props.descriptor.values ?? []).map(v => ({ label: String(v), value: String(v) }))]);
/** Tags typed as "a, b" become two tags, without duplicates. */
function setTags(tags: unknown[]) {
  set([...new Set(tags.flatMap(t => String(t).split(",")).map(t => t.trim()).filter(Boolean))]);
}
const stringList = computed(() => props.descriptor.type === "array" && props.descriptor.items?.type === "string");
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
/** A dropped or picked file, handed to the dialog for its alt text. */
const imageFile = ref<File | null>(null);
const pendingPreview = computed(() => typeof props.modelValue === "string" && props.modelValue.startsWith("blob:"));
const imageSrc = computed(() => {
  const v = props.modelValue;
  if (typeof v !== "string" || !v) return null;
  return pendingPreview.value || !registry?.resolve ? v : registry.resolve(v);
});
function uploadImage(file: File) { imageFile.value = file; imageOpen.value = true; }
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
const referenceItems = computed<Item[]>(() => [
  ...notSet.value,
  ...(referenceIds.value ?? []).map(entryId => ({ label: entryId, value: entryId })),
  ...(missingReference.value ? [{ label: `${missingReference.value} (missing)`, value: missingReference.value }] : []),
]);

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
function chooseVariant(value: unknown) {
  const name = String(value);
  const next = (props.descriptor.variants ?? []).find(v => String(v.properties?.[props.descriptor.discriminator!]?.values?.[0]) === name);
  set(next ? { ...(empty(next) as object), [props.descriptor.discriminator!]: name } : {});
}
/** The field name shown to the writer (`coverAlt` → "Cover alt"); list items have no label of their own. */
const title = computed(() => {
  if (!props.label) return undefined;
  // A document's own title is what the page shows; its file name is the slug, shown above the form.
  if (props.label === "title" && props.path.length === 1) return "Display title";
  return humanize(props.label);
});
const phrase = computed(() => (props.label ? humanizeLower(props.label) : undefined));
const placeholder = computed(() => (phrase.value ? `Enter ${phrase.value}` : "Enter a value"));
</script>

<template>
  <div class="mb-3" :data-testid="`field-wrap-${key}`">
    <UFormField
      :label="label && descriptor.type !== 'boolean' ? title : undefined"
      :required="!optional"
      :error="message ? `${key}: ${message}` : undefined"
      :ui="{ label: 'text-xs', container: 'mt-1.5' }"
    >
      <template #error="{ error }">
        <span role="alert" :data-testid="`${id}-error`">{{ error }}</span>
      </template>

      <template v-if="descriptor.type === 'unsupported'">
        <p class="text-muted text-xs" :data-testid="`${id}-readonly`">Read only: {{ descriptor.reason }}</p>
        <pre v-if="modelValue !== undefined" class="whitespace-pre-wrap text-xs">{{ JSON.stringify(modelValue, null, 2) }}</pre>
      </template>

      <template v-else-if="descriptor.type === 'literal'">
        <p :id="id" class="text-muted text-sm">{{ descriptor.values?.[0] }}</p>
      </template>

      <UCheckbox v-else-if="descriptor.type === 'boolean'" :id="id" :data-testid="id" :label="title" :model-value="modelValue === true" @update:model-value="set($event === true)" />

      <UInput v-else-if="descriptor.type === 'number'" :id="id" :data-testid="id" type="number" class="w-full" :placeholder="placeholder" :model-value="modelValue as number | undefined" @update:model-value="setNumber" />

      <LabelTarget v-else-if="descriptor.type === 'date'" :id="id">
        <DatePicker :id="id" :data-testid="id" :model-value="modelValue as string | undefined" :clearable="optional" @update:model-value="setText" />
      </LabelTarget>

      <USelect v-else-if="descriptor.type === 'enum'" :id="id" :data-testid="id" class="w-full" :items="enumItems" :placeholder="optional ? 'Not set' : phrase ? `Choose ${phrase}` : 'Choose a value'" :model-value="choice(modelValue)" @update:model-value="setChoice" />

      <UInputTags v-else-if="stringList" :id="id" :data-testid="id" class="w-full" :model-value="listValue.map(String)" :placeholder="`Add ${phrase ?? 'items'}, press Enter`" add-on-blur @update:model-value="setTags" />

      <template v-else-if="descriptor.type === 'array'">
        <div v-for="(item, index) in listValue" :key="index" class="mb-2 flex items-start gap-1" :data-testid="`${id}-item`">
          <div class="min-w-0 flex-1 [&>*]:mb-0">
            <FrontmatterForm :descriptor="descriptor.items!" :model-value="item" :path="[...path, index]" :errors="errors" @update:model-value="setItem(index, $event)" />
          </div>
          <UButton type="button" color="neutral" variant="outline" square icon="i-hugeicons-cancel-01" class="shrink-0" aria-label="Remove item" :data-testid="`${id}-remove-${index}`" @click="set(listValue.filter((_, i) => i !== index))" />
        </div>
        <UButton type="button" color="neutral" variant="outline" size="sm" icon="i-hugeicons-plus-sign" label="Add item" :data-testid="`${id}-add`" @click="set([...listValue, empty(descriptor.items)])" />
      </template>

      <fieldset v-else-if="descriptor.type === 'object'" :class="path.length ? 'rounded-md border border-default p-3' : ''">
        <FrontmatterForm v-for="(child, name) in descriptor.properties" :key="name" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, descriptor.properties)" />
      </fieldset>

      <fieldset v-else-if="descriptor.type === 'union'" class="rounded-md border border-default p-3">
        <USelect :id="id" :data-testid="id" class="w-full" :items="variantItems" :placeholder="`Choose ${descriptor.discriminator}`" :model-value="choice(objectValue[descriptor.discriminator!])" @update:model-value="chooseVariant" />
        <template v-if="variant">
          <template v-for="(child, name) in variant.properties" :key="name">
            <FrontmatterForm v-if="name !== descriptor.discriminator" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" class="mt-3" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, variant.properties)" />
          </template>
        </template>
      </fieldset>

      <USelect v-else-if="descriptor.type === 'reference' && referenceIds" :id="id" :data-testid="id" class="w-full" :items="referenceItems" :placeholder="optional ? 'Not set' : `Choose a ${descriptor.collection} entry`" :model-value="choice(modelValue)" @update:model-value="setChoice" />
      <UInput v-else-if="descriptor.type === 'reference'" :id="id" :data-testid="id" type="text" class="w-full" :placeholder="`${descriptor.collection} entry`" :model-value="modelValue as string | undefined" @update:model-value="setText" />
      <LabelTarget v-else-if="descriptor.type === 'image'" :id="id">
        <ImageUploader :id="id" :data-testid="`${id}-uploader`" :src="imageSrc" :accept="ACCEPTED_IMAGES" @select="uploadImage" @remove="setText('')">
          <template #hint><span class="text-xs">PNG, JPEG, WebP, or GIF up to 10 MiB</span></template>
        </ImageUploader>
        <ImageDialog v-model:open="imageOpen" :file="imageFile" @insert="chooseImage" />
      </LabelTarget>

      <UInput v-else :id="id" :data-testid="id" type="text" class="w-full" :placeholder="placeholder" :model-value="modelValue as string | undefined" @update:model-value="setText" />
    </UFormField>
  </div>
</template>
