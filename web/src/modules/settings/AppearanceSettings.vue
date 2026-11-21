<script setup lang="ts">
// App settings → Appearance. The theme and typing sound are read and written by
// the same shared composables the rest of the app uses (and the command palette).
import { computed } from "vue";
import UButton from "@nuxt/ui/components/Button.vue";
import USelect from "@nuxt/ui/components/Select.vue";
import USwitch from "@nuxt/ui/components/Switch.vue";
import { useChatPanelLayout, type ChatPanelLayout } from "@/lib/chatPanelLayout";
import { useChatSteps } from "@/lib/chatSteps";
import { useTheme, type ThemePreference } from "@/lib/theme";
import {
  SOUND_PACKS,
  useTypingSound,
  type SoundPack,
  type TypingSoundSetting,
} from "@/lib/typingSound";
import SettingsRow from "./SettingsRow.vue";
import SettingsSection from "./SettingsSection.vue";
import ThemePreview from "./ThemePreview.vue";

const theme = useTheme();
const typingSound = useTypingSound();
const chatSteps = useChatSteps();
const panelLayout = useChatPanelLayout();

const LAYOUT_OPTIONS: { value: ChatPanelLayout; label: string }[] = [
  { value: "sidebar", label: "Sidebar" },
  { value: "floating", label: "Floating" },
];
const layoutValue = computed(() => panelLayout.layout.value);

function onLayout(value: unknown) {
  if (value === "sidebar" || value === "floating") {
    panelLayout.set(value);
  }
}

const THEMES: { id: ThemePreference; label: string }[] = [
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
  { id: "system", label: "Auto" },
];
const themeValue = computed(() => theme.preference.value);

const SOUND_OPTIONS: { value: TypingSoundSetting; label: string }[] = [
  { value: "off", label: "Off" },
  ...(Object.keys(SOUND_PACKS) as SoundPack[]).map((id) => ({ value: id, label: SOUND_PACKS[id].label })),
];
const soundValue = computed(() => typingSound.setting.value);

function onTheme(value: ThemePreference) {
  theme.setPreference(value);
}

function onSound(value: unknown) {
  if (value === "off" || (typeof value === "string" && value in SOUND_PACKS)) {
    typingSound.set(value as TypingSoundSetting);
  }
}

/** Replaying the current choice is the sample: choosing a pack already plays one. */
function playSample() {
  if (soundValue.value === "off") return;
  typingSound.set(soundValue.value);
}
</script>

<template>
  <div class="flex flex-col gap-5" data-testid="appearance-settings">
    <SettingsSection label="Light and dark" icon="i-hugeicons-sun-cloud-01">
      <SettingsRow
        icon="i-hugeicons-sun-cloud-01"
        title="Appearance"
        description="Light, dark, or whatever the Mac is doing."
        align="start"
      >
        <div role="group" aria-label="Appearance" class="mt-1.5 grid w-full grid-cols-3 gap-3">
          <UButton
            v-for="option in THEMES"
            :key="option.id"
            color="neutral"
            variant="ghost"
            size="xs"
            block
            :active="themeValue === option.id"
            :aria-pressed="themeValue === option.id"
            :data-testid="`appearance-theme-${option.id}`"
            :ui="{ base: 'flex-col gap-1.5 p-1 hover:bg-transparent active:bg-transparent' }"
            @click="onTheme(option.id)"
          >
            <ThemePreview
              :mode="option.id"
              class="aspect-video w-full transition-shadow"
              :class="themeValue === option.id ? 'ring-2 ring-primary' : 'hover:ring-1 hover:ring-accented'"
            />
            <span :class="themeValue === option.id ? 'text-highlighted font-medium' : 'text-muted'">
              {{ option.label }}
            </span>
          </UButton>
        </div>
      </SettingsRow>
    </SettingsSection>

    <SettingsSection label="Typing" icon="i-hugeicons-audio-wave-01">
      <SettingsRow
        icon="i-hugeicons-audio-wave-01"
        title="Typing sound"
        description="Plays a soft key sound while you type in the content editor."
      >
        <template #actions>
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-hugeicons-volume-high"
            label="Play sample"
            :disabled="soundValue === 'off'"
            data-testid="appearance-play-sample"
            @click="playSample"
          />
          <USelect
            id="appearance-typing-sound"
            :model-value="soundValue"
            :items="SOUND_OPTIONS"
            size="sm"
            class="w-36"
            aria-label="Typing sound"
            data-testid="appearance-typing-sound"
            @update:model-value="onSound"
          />
        </template>
      </SettingsRow>
    </SettingsSection>

    <SettingsSection label="Chat" icon="i-hugeicons-check-list">
      <SettingsRow
        icon="i-hugeicons-sidebar-right"
        title="Agent panel"
        description="Dock the chat in a sidebar, or float it over the page with a bottom bar like Linear."
      >
        <template #actions>
          <USelect
            id="appearance-panel-layout"
            :model-value="layoutValue"
            :items="LAYOUT_OPTIONS"
            size="sm"
            class="w-36"
            aria-label="Agent panel"
            data-testid="appearance-panel-layout"
            @update:model-value="onLayout"
          />
        </template>
      </SettingsRow>
      <SettingsRow
        icon="i-hugeicons-check-list"
        title="Show assistant steps"
        description="Shows “Worked through N steps” under each reply. Off by default for a quieter chat."
      >
        <template #actions>
          <USwitch
            :model-value="chatSteps.enabled.value"
            aria-label="Show assistant steps"
            data-testid="appearance-show-steps"
            @update:model-value="chatSteps.set"
          />
        </template>
      </SettingsRow>
    </SettingsSection>
  </div>
</template>
