<script setup lang="ts">
import { reactive, toRaw, watch } from "vue";
import type { AccordionItem } from "@nuxt/ui";
import UAccordion from "@nuxt/ui/components/Accordion.vue";
import UButton from "@nuxt/ui/components/Button.vue";
import UCard from "@nuxt/ui/components/Card.vue";
import UFormField from "@nuxt/ui/components/FormField.vue";
import UInput from "@nuxt/ui/components/Input.vue";
import type { DesignTokens } from "@/core/types";
import { addKey, removeKey, renameKey } from "./tokens";

defineOptions({ name: "DesignTokensForm" });
const props = defineProps<{ modelValue: DesignTokens }>();
const emit = defineEmits<{ "update:modelValue": [value: DesignTokens] }>();

type Group = Record<string, unknown>;
const GROUPS = ["brand", "colors", "typography", "rounded", "spacing"];
const DIMENSIONS = [{ group: "rounded", title: "Rounded", add: "Add radius" }, { group: "spacing", title: "Spacing", add: "Add spacing" }];
const SECTIONS: AccordionItem[] = [
  { label: "Brand", value: "brand", slot: "brand" },
  { label: "Colors", value: "colors", slot: "colors" },
  { label: "Typography", value: "typography", slot: "typography" },
  ...DIMENSIONS.map(({ group, title, add }) => ({ label: title, value: group, slot: "dimension", group, add })),
];
const TYPOGRAPHY_FIELDS = [
  ["fontFamily", "Font family"],
  ["fontSize", "Font size"],
  ["fontWeight", "Font weight"],
  ["lineHeight", "Line height"],
  ["letterSpacing", "Letter spacing"],
] as const;

const isObject = (value: unknown): value is Group => !!value && typeof value === "object" && !Array.isArray(value);
const groupOf = (name: string): Group => (isObject(props.modelValue[name]) ? props.modelValue[name] : {});
const text = (value: unknown) => (value === undefined || value === null ? "" : String(value));

/** Replaces one top-level key; everything else, shown or not, is carried over as it was. */
function setKey(name: string, value: unknown) {
  lastEmitted = { ...props.modelValue, [name]: value };
  emit("update:modelValue", lastEmitted);
}
const setValue = (group: string, key: string, value: unknown) => setKey(group, { ...groupOf(group), [key]: value });
function remove(group: string, key: string) {
  forget(group, key);
  setKey(group, removeKey(groupOf(group), key));
}
const add = (group: string, base: string, value: unknown) => setKey(group, addKey(groupOf(group), base, value));

// Renaming is committed on blur or Enter, so half-typed names never reach the draft.
const drafts = reactive<Record<string, string>>({});
const errors = reactive<Record<string, string>>({});
const slot = (group: string, key: string) => `${group}.${key}`;
/** Drops every pending draft and message for a token (it was removed or renamed). */
function forget(group: string, key: string) {
  delete drafts[slot(group, key)];
  delete errors[slot(group, key)];
  if (group === "typography") for (const field of Object.keys(PARSERS)) delete fieldDrafts[fieldSlot(key, field)];
}
function typeName(group: string, key: string, value: string | number) {
  drafts[slot(group, key)] = String(value);
  delete errors[slot(group, key)];
}
function commitName(group: string, key: string) {
  const id = slot(group, key);
  const to = drafts[id]?.trim();
  if (to === undefined) return;
  delete drafts[id];
  if (to === key) return;
  const renamed = renameKey(groupOf(group), key, to);
  if (renamed) {
    forget(group, key);
    setKey(group, renamed);
  } else errors[id] = "Name already used";
}

const HEX = /^#[0-9a-f]{6}$/i;
const SHORT_HEX = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i;
/** `<input type="color">` only takes `#rrggbb`. */
function pickerValue(value: unknown) {
  const css = text(value);
  const short = SHORT_HEX.exec(css);
  if (short) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`;
  return HEX.test(css) ? css : "#000000";
}

// fontWeight and lineHeight have a schema, so their text stays in a local draft and only valid values are emitted.
const NUMBER = /^(\d+(\.\d+)?|\.\d+)$/;
const DIMENSION = /^(\d+(\.\d+)?|\.\d+)[a-z%]+$/i;
const PARSERS: Record<string, (input: string) => number | string | undefined> = {
  fontWeight: input => (NUMBER.test(input) ? Number(input) : undefined),
  lineHeight: input => (NUMBER.test(input) ? Number(input) : DIMENSION.test(input) ? input : undefined),
};
const fieldDrafts = reactive<Record<string, string>>({});
const fieldSlot = (name: string, field: string) => JSON.stringify([name, field]);
const fieldText = (name: string, field: string) => fieldDrafts[fieldSlot(name, field)] ?? text(style(name)[field]);
function setTypography(name: string, field: string, raw: string | number) {
  const input = String(raw);
  const parse = PARSERS[field];
  let value: unknown = input;
  if (parse) {
    fieldDrafts[fieldSlot(name, field)] = input;
    value = parse(input.trim());
    if (value === undefined) return;
  }
  const { [field]: _old, ...rest } = style(name);
  setValue("typography", name, field === "letterSpacing" && input === "" ? rest : { ...rest, [field]: value });
}

// Drafts only belong to the value this form emitted; a value set from outside (reload, discard) replaces them.
let lastEmitted: DesignTokens | null = null;
watch(() => props.modelValue, value => {
  if (lastEmitted && toRaw(value) === toRaw(lastEmitted)) return;
  for (const draftMap of [drafts, errors, fieldDrafts]) for (const key of Object.keys(draftMap)) delete draftMap[key];
});
/** Leaving the field drops an invalid draft, so the input shows the last valid value again. */
function endTypography(name: string, field: string) { delete fieldDrafts[fieldSlot(name, field)]; }
const style = (name: string) => (isObject(groupOf("typography")[name]) ? groupOf("typography")[name] as Group : {});
const newStyle = { fontFamily: "Inter", fontSize: "16px", fontWeight: 400, lineHeight: 1.5 };
</script>

<template>
  <!-- Content stays mounted when a group is collapsed, so every group keeps its test id and drafts. -->
  <UAccordion type="multiple" :items="SECTIONS" :default-value="GROUPS" :unmount-on-hide="false" class="w-full">
    <template #brand-body>
      <div class="grid gap-3" data-testid="design-group-brand">
        <UFormField label="Name">
          <UInput id="design-brand-name" class="w-full" data-testid="design-brand-name" type="text" :model-value="text(modelValue.name)" @update:model-value="setKey('name', $event)" />
        </UFormField>
        <UFormField label="Description">
          <UInput id="design-brand-description" class="w-full" data-testid="design-brand-description" type="text" :model-value="text(modelValue.description)" @update:model-value="setKey('description', $event)" />
        </UFormField>
      </div>
    </template>

    <template #colors-body>
      <div data-testid="design-group-colors">
        <div v-for="(value, key) in groupOf('colors')" :key="key" class="mb-2">
          <UFormField :error="!!errors[slot('colors', String(key))]">
            <div class="flex items-center gap-1">
              <UInput type="color" class="shrink-0" :ui="{ base: 'size-8 cursor-pointer p-0.5' }" :aria-label="`Pick colour for ${key}`" :data-testid="`design-colors-picker-${key}`" :model-value="pickerValue(value)" @update:model-value="setValue('colors', String(key), String($event))" />
              <!-- The swatch shows the token's own CSS value (any colour syntax), so it needs an inline style. -->
              <span class="size-5 shrink-0 rounded-full border border-default" aria-hidden="true" :data-testid="`design-colors-swatch-${key}`" :style="{ background: text(value) }" />
              <UInput :id="`design-colors-name-${key}`" type="text" class="min-w-0 flex-1" :aria-label="`Colour name ${key}`" :data-testid="`design-colors-name-${key}`" :model-value="drafts[slot('colors', String(key))] ?? String(key)" @update:model-value="typeName('colors', String(key), $event)" @blur="commitName('colors', String(key))" @keydown.enter="commitName('colors', String(key))" />
              <UInput :id="`design-colors-value-${key}`" type="text" class="min-w-0 flex-1" :aria-label="`Colour value of ${key}`" :data-testid="`design-colors-value-${key}`" :model-value="text(value)" @update:model-value="setValue('colors', String(key), $event)" />
              <UButton type="button" color="neutral" variant="outline" icon="i-hugeicons-cancel-01" class="shrink-0" :aria-label="`Remove colour ${key}`" :data-testid="`design-colors-remove-${key}`" @click="remove('colors', String(key))" />
            </div>
          </UFormField>
          <p v-if="errors[slot('colors', String(key))]" class="text-error mt-1 text-xs" role="alert" :data-testid="`design-colors-error-${key}`">{{ errors[slot('colors', String(key))] }}</p>
        </div>
        <UButton type="button" color="neutral" variant="outline" size="sm" icon="i-hugeicons-plus-sign" label="Add colour" data-testid="design-colors-add" @click="add('colors', 'color', '#000000')" />
      </div>
    </template>

    <template #typography-body>
      <div data-testid="design-group-typography">
        <UCard v-for="(_, key) in groupOf('typography')" :key="key" as="fieldset" class="mb-3" :ui="{ body: 'grid gap-2 p-3 sm:p-3' }">
          <UFormField :error="!!errors[slot('typography', String(key))]">
            <div class="flex items-center gap-1">
              <UInput type="text" class="min-w-0 flex-1" :aria-label="`Style name ${key}`" :data-testid="`design-typography-name-${key}`" :model-value="drafts[slot('typography', String(key))] ?? String(key)" @update:model-value="typeName('typography', String(key), $event)" @blur="commitName('typography', String(key))" @keydown.enter="commitName('typography', String(key))" />
              <UButton type="button" color="neutral" variant="outline" icon="i-hugeicons-cancel-01" class="shrink-0" :aria-label="`Remove style ${key}`" :data-testid="`design-typography-remove-${key}`" @click="remove('typography', String(key))" />
            </div>
          </UFormField>
          <p v-if="errors[slot('typography', String(key))]" class="text-error text-xs" role="alert" :data-testid="`design-typography-error-${key}`">{{ errors[slot('typography', String(key))] }}</p>
          <UFormField v-for="[field, label] in TYPOGRAPHY_FIELDS" :key="field" :label="label" size="sm">
            <UInput :id="`design-typography-${field}-${key}`" type="text" size="md" class="w-full" :data-testid="`design-typography-${field}-${key}`" :model-value="fieldText(String(key), field)" @update:model-value="setTypography(String(key), field, $event)" @blur="endTypography(String(key), field)" />
          </UFormField>
        </UCard>
        <UButton type="button" color="neutral" variant="outline" size="sm" icon="i-hugeicons-plus-sign" label="Add style" data-testid="design-typography-add" @click="add('typography', 'style', { ...newStyle })" />
      </div>
    </template>

    <template #dimension-body="{ item: { group, label: title, add: addLabel } }">
      <div :data-testid="`design-group-${group}`">
        <div v-for="(value, key) in groupOf(group)" :key="key" class="mb-2">
          <UFormField :error="!!errors[slot(group, String(key))]">
            <div class="flex items-center gap-1">
              <UInput :id="`design-${group}-name-${key}`" type="text" class="min-w-0 flex-1" :aria-label="`${title} name ${key}`" :data-testid="`design-${group}-name-${key}`" :model-value="drafts[slot(group, String(key))] ?? String(key)" @update:model-value="typeName(group, String(key), $event)" @blur="commitName(group, String(key))" @keydown.enter="commitName(group, String(key))" />
              <UInput :id="`design-${group}-value-${key}`" type="text" class="min-w-0 flex-1" :aria-label="`${title} value of ${key}`" :data-testid="`design-${group}-value-${key}`" :model-value="text(value)" @update:model-value="setValue(group, String(key), $event)" />
              <UButton type="button" color="neutral" variant="outline" icon="i-hugeicons-cancel-01" class="shrink-0" :aria-label="`Remove ${key}`" :data-testid="`design-${group}-remove-${key}`" @click="remove(group, String(key))" />
            </div>
          </UFormField>
          <p v-if="errors[slot(group, String(key))]" class="text-error mt-1 text-xs" role="alert" :data-testid="`design-${group}-error-${key}`">{{ errors[slot(group, String(key))] }}</p>
        </div>
        <UButton type="button" color="neutral" variant="outline" size="sm" icon="i-hugeicons-plus-sign" :label="addLabel" :data-testid="`design-${group}-add`" @click="add(group, 'size', '8px')" />
      </div>
    </template>
  </UAccordion>
</template>
