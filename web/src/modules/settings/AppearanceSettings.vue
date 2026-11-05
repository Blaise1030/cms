<script setup lang="ts">
// App settings → Appearance. Both preferences are read and written by the same
// shared composables the rest of the app uses (and the command palette).
import { computed } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { THEME_LABELS, useTheme, type ThemePreference } from "@/lib/theme";
import {
  SOUND_PACKS,
  useTypingSound,
  type SoundPack,
  type TypingSoundSetting,
} from "@/lib/typingSound";

const theme = useTheme();
const typingSound = useTypingSound();

const THEMES: ThemePreference[] = ["light", "dark", "system"];
const themeValue = computed(() => theme.preference.value);

const SOUND_OPTIONS: { id: TypingSoundSetting; label: string }[] = [
  ...(Object.keys(SOUND_PACKS) as SoundPack[]).map((id) => ({ id, label: SOUND_PACKS[id].label })),
  { id: "off", label: "Off" },
];
const soundValue = computed(() => typingSound.setting.value);

function onTheme(value: unknown) {
  if (value === "light" || value === "dark" || value === "system") theme.setPreference(value);
}

function onSound(value: unknown) {
  if (value === "off" || (typeof value === "string" && value in SOUND_PACKS)) {
    typingSound.set(value as TypingSoundSetting);
  }
}
</script>

<template>
  <Card data-testid="appearance-settings">
    <CardHeader>
      <CardTitle>Appearance</CardTitle>
      <CardDescription>How orso looks and sounds while you work.</CardDescription>
    </CardHeader>

    <CardContent>
      <FieldGroup class="gap-6">
        <Field>
          <FieldLabel for="appearance-theme">Theme</FieldLabel>
          <Select :model-value="themeValue" @update:model-value="onTheme">
            <SelectTrigger id="appearance-theme" data-testid="appearance-theme" class="w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="mode in THEMES" :key="mode" :value="mode">
                {{ THEME_LABELS[mode] }}
              </SelectItem>
            </SelectContent>
          </Select>
          <FieldDescription>System follows your Mac's light or dark appearance.</FieldDescription>
        </Field>

        <Separator />

        <Field>
          <FieldLabel for="appearance-typing-sound">Typing sound</FieldLabel>
          <Select :model-value="soundValue" @update:model-value="onSound">
            <SelectTrigger
              id="appearance-typing-sound"
              data-testid="appearance-typing-sound"
              class="w-48"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="option in SOUND_OPTIONS" :key="option.id" :value="option.id">
                {{ option.label }}
              </SelectItem>
            </SelectContent>
          </Select>
          <FieldDescription>
            Plays a sample when you choose a pack. It only sounds in the content editor.
          </FieldDescription>
        </Field>
      </FieldGroup>
    </CardContent>
  </Card>
</template>
