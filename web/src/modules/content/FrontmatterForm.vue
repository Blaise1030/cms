<script setup lang="ts">
import { computed, inject, ref, watchEffect } from "vue";
import { Plus, X } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DatePicker } from "@/components/ui/date-picker";
import { ImageUploader } from "@/components/ui/image-uploader";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { TagInput } from "@/components/ui/tag-input";
import type { FieldDescriptor } from "@/core/types";
import { humanize, humanizeLower } from "@workbench/utils";
import ImageDialog, { ACCEPTED_IMAGES, IMAGE_REGISTRY, type PendingImage } from "./ImageDialog.vue";
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
const title = computed(() => (props.label ? humanize(props.label) : undefined));
const phrase = computed(() => (props.label ? humanizeLower(props.label) : undefined));
const placeholder = computed(() => (phrase.value ? `Enter ${phrase.value}` : "Enter a value"));
</script>

<template>
  <div class="mb-3" :data-testid="`field-wrap-${key}`">
    <Label v-if="label && descriptor.type !== 'boolean'" :for="id" class="mb-1.5 text-xs">{{ title }}<span v-if="!optional" class="text-destructive">*</span></Label>

    <template v-if="descriptor.type === 'unsupported'">
      <p class="text-xs text-muted-foreground" :data-testid="`${id}-readonly`">Read only: {{ descriptor.reason }}</p>
      <pre v-if="modelValue !== undefined" class="whitespace-pre-wrap text-xs">{{ JSON.stringify(modelValue, null, 2) }}</pre>
    </template>

    <template v-else-if="descriptor.type === 'literal'">
      <p :id="id" class="text-sm text-muted-foreground">{{ descriptor.values?.[0] }}</p>
    </template>

    <div v-else-if="descriptor.type === 'boolean'" class="flex items-center gap-2">
      <Checkbox :id="id" :data-testid="id" :model-value="modelValue === true" @update:model-value="set($event === true)" />
      <Label :for="id" class="text-sm font-normal">{{ title }}</Label>
    </div>

    <Input v-else-if="descriptor.type === 'number'" :id="id" :data-testid="id" type="number" :placeholder="placeholder" :model-value="modelValue as number | undefined" @update:model-value="setNumber" />

    <DatePicker v-else-if="descriptor.type === 'date'" :id="id" :data-testid="id" :model-value="modelValue as string | undefined" :clearable="optional" @update:model-value="setText" />

    <Select v-else-if="descriptor.type === 'enum'" :model-value="choice(modelValue)" @update:model-value="setChoice">
      <SelectTrigger :id="id" :data-testid="id" class="w-full"><SelectValue :placeholder="optional ? 'Not set' : phrase ? `Choose ${phrase}` : 'Choose a value'" /></SelectTrigger>
      <SelectContent>
        <SelectItem v-if="optional" :value="UNSET">Not set</SelectItem>
        <SelectItem v-for="v in descriptor.values" :key="String(v)" :value="String(v)">{{ v }}</SelectItem>
      </SelectContent>
    </Select>

    <TagInput v-else-if="stringList" :id="id" :data-testid="id" :model-value="listValue.map(String)" :placeholder="`Add ${phrase ?? 'items'}, press Enter`" @update:model-value="set" />

    <template v-else-if="descriptor.type === 'array'">
      <div v-for="(item, index) in listValue" :key="index" class="mb-2 flex items-start gap-1" :data-testid="`${id}-item`">
        <div class="min-w-0 flex-1 [&>*]:mb-0">
          <FrontmatterForm :descriptor="descriptor.items!" :model-value="item" :path="[...path, index]" :errors="errors" @update:model-value="setItem(index, $event)" />
        </div>
        <Button type="button" variant="outline" size="icon" class="size-9 shrink-0" aria-label="Remove item" :data-testid="`${id}-remove-${index}`" @click="set(listValue.filter((_, i) => i !== index))"><X /></Button>
      </div>
      <Button type="button" variant="outline" size="sm" :data-testid="`${id}-add`" @click="set([...listValue, empty(descriptor.items)])"><Plus /> Add item</Button>
    </template>

    <fieldset v-else-if="descriptor.type === 'object'" :class="path.length ? 'rounded-md border p-3' : ''">
      <FrontmatterForm v-for="(child, name) in descriptor.properties" :key="name" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, descriptor.properties)" />
    </fieldset>

    <fieldset v-else-if="descriptor.type === 'union'" class="rounded-md border p-3">
      <Select :model-value="choice(objectValue[descriptor.discriminator!])" @update:model-value="chooseVariant">
        <SelectTrigger :id="id" :data-testid="id" class="w-full"><SelectValue :placeholder="`Choose ${descriptor.discriminator}`" /></SelectTrigger>
        <SelectContent>
          <SelectItem v-for="name in variantNames" :key="name" :value="name">{{ name }}</SelectItem>
        </SelectContent>
      </Select>
      <template v-if="variant">
        <template v-for="(child, name) in variant.properties" :key="name">
          <FrontmatterForm v-if="name !== descriptor.discriminator" :descriptor="child" :label="String(name)" :model-value="objectValue[name]" :path="[...path, name]" :errors="errors" class="mt-3" @update:model-value="setChild(String(name), $event)" @image="setImage(String(name), $event, variant.properties)" />
        </template>
      </template>
    </fieldset>

    <Select v-else-if="descriptor.type === 'reference' && referenceIds" :model-value="choice(modelValue)" @update:model-value="setChoice">
      <SelectTrigger :id="id" :data-testid="id" class="w-full"><SelectValue :placeholder="optional ? 'Not set' : `Choose a ${descriptor.collection} entry`" /></SelectTrigger>
      <SelectContent>
        <SelectItem v-if="optional" :value="UNSET">Not set</SelectItem>
        <SelectItem v-for="entryId in referenceIds" :key="entryId" :value="entryId">{{ entryId }}</SelectItem>
        <SelectItem v-if="missingReference" :value="missingReference">{{ missingReference }} (missing)</SelectItem>
      </SelectContent>
    </Select>
    <Input v-else-if="descriptor.type === 'reference'" :id="id" :data-testid="id" type="text" :placeholder="`${descriptor.collection} entry`" :model-value="modelValue as string | undefined" @update:model-value="setText" />
    <div v-else-if="descriptor.type === 'image'">
      <ImageUploader :id="id" :data-testid="`${id}-uploader`" :src="imageSrc" :accept="ACCEPTED_IMAGES" @select="uploadImage" @remove="setText('')">
        <template #hint><span class="text-xs">PNG, JPEG, WebP, or GIF up to 10 MiB</span></template>
      </ImageUploader>
      <ImageDialog v-model:open="imageOpen" :file="imageFile" @insert="chooseImage" />
    </div>

    <Input v-else :id="id" :data-testid="id" type="text" :placeholder="placeholder" :model-value="modelValue as string | undefined" @update:model-value="setText" />

    <p v-if="message" class="mt-1 text-xs text-destructive" role="alert" :data-testid="`${id}-error`">{{ key }}: {{ message }}</p>
  </div>
</template>
