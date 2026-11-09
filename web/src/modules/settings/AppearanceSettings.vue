<script setup lang="ts">
// App settings → Appearance. The theme and typing sound are read and written by
// the same shared composables the rest of the app uses (and the command palette).
import { computed } from "vue";
import { AudioLines, ListChecks, PanelRight, SunMoon, Volume2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  useChatPanelLayout,
  type ChatPanelLayout,
} from "@/lib/chatPanelLayout";
import { useChatSteps } from "@/lib/chatSteps";
import { useTheme, type ThemePreference } from "@/lib/theme";
import {
  SOUND_PACKS,
  useTypingSound,
  type SoundPack,
  type TypingSoundSetting,
} from "@/lib/typingSound";
import SettingsSection from "./SettingsSection.vue";
import ThemePreview from "./ThemePreview.vue";

const theme = useTheme();
const typingSound = useTypingSound();
const chatSteps = useChatSteps();
const panelLayout = useChatPanelLayout();

const LAYOUT_OPTIONS: { id: ChatPanelLayout; label: string }[] = [
  { id: "sidebar", label: "Sidebar" },
  { id: "floating", label: "Floating" },
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

const SOUND_OPTIONS: { id: TypingSoundSetting; label: string }[] = [
  { id: "off", label: "Off" },
  ...(Object.keys(SOUND_PACKS) as SoundPack[]).map((id) => ({ id, label: SOUND_PACKS[id].label })),
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
  <div class="flex flex-col gap-8" data-testid="appearance-settings">
    <SettingsSection label="Light and dark" :icon="SunMoon">
      <Card class="gap-0 overflow-hidden p-0">
        <Item class="items-start">
          <ItemMedia variant="icon">
            <SunMoon aria-hidden="true" />
          </ItemMedia>
          <ItemContent class="gap-3">
            <div class="flex flex-col gap-1">
              <ItemTitle>Appearance</ItemTitle>
              <ItemDescription>Light, dark, or whatever the Mac is doing.</ItemDescription>
            </div>

            <div role="group" aria-label="Appearance" class="flex flex-wrap gap-3">
              <button
                v-for="option in THEMES"
                :key="option.id"
                type="button"
                :aria-pressed="themeValue === option.id"
                :data-testid="`appearance-theme-${option.id}`"
                class="group/theme flex flex-col items-center gap-2 rounded-lg p-1 outline-none focus-visible:ring-2 focus-visible:ring-ring"
                @click="onTheme(option.id)"
              >
                <span
                  class="rounded-md p-0.5 transition-shadow"
                  :class="
                    themeValue === option.id
                      ? 'ring-2 ring-ring'
                      : 'group-hover/theme:ring-1 group-hover/theme:ring-ring/40'
                  "
                >
                  <ThemePreview :mode="option.id" />
                </span>
                <span
                  class="text-xs"
                  :class="themeValue === option.id ? 'text-foreground font-medium' : 'text-muted-foreground'"
                >
                  {{ option.label }}
                </span>
              </button>
            </div>
          </ItemContent>
        </Item>
      </Card>
    </SettingsSection>

    <SettingsSection label="Typing" :icon="AudioLines">
      <Card class="gap-0 overflow-hidden p-0">
        <Item>
          <ItemMedia variant="icon">
            <AudioLines aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>Typing sound</ItemTitle>
            <ItemDescription>
              Plays a soft key sound while you type in the content editor.
            </ItemDescription>
          </ItemContent>
          <ItemActions class="gap-2">
            <Button
              variant="ghost"
              size="sm"
              :disabled="soundValue === 'off'"
              data-testid="appearance-play-sample"
              @click="playSample"
            >
              <Volume2 aria-hidden="true" /> Play sample
            </Button>
            <Select :model-value="soundValue" @update:model-value="onSound">
              <SelectTrigger
                id="appearance-typing-sound"
                data-testid="appearance-typing-sound"
                class="w-40"
                aria-label="Typing sound"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="option in SOUND_OPTIONS" :key="option.id" :value="option.id">
                  {{ option.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </ItemActions>
        </Item>
      </Card>
    </SettingsSection>

    <SettingsSection label="Chat" :icon="ListChecks">
      <Card class="gap-0 overflow-hidden p-0">
        <div class="divide-y divide-border">
          <Item>
            <ItemMedia variant="icon">
              <PanelRight aria-hidden="true" />
            </ItemMedia>
            <ItemContent>
              <ItemTitle>Agent panel</ItemTitle>
              <ItemDescription>
                Dock the chat in a sidebar, or float it over the page with a bottom bar like Linear.
              </ItemDescription>
            </ItemContent>
            <ItemActions>
              <Select :model-value="layoutValue" @update:model-value="onLayout">
                <SelectTrigger
                  id="appearance-panel-layout"
                  data-testid="appearance-panel-layout"
                  class="w-40"
                  aria-label="Agent panel"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="option in LAYOUT_OPTIONS" :key="option.id" :value="option.id">
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </ItemActions>
          </Item>
          <Item>
            <ItemMedia variant="icon">
              <ListChecks aria-hidden="true" />
            </ItemMedia>
            <ItemContent>
              <ItemTitle>Show assistant steps</ItemTitle>
              <ItemDescription>
                Shows “Worked through N steps” under each reply. Off by default for a quieter chat.
              </ItemDescription>
            </ItemContent>
            <ItemActions>
              <Switch
                :model-value="chatSteps.enabled.value"
                data-testid="appearance-show-steps"
                aria-label="Show assistant steps"
                @update:model-value="chatSteps.set"
              />
            </ItemActions>
          </Item>
        </div>
      </Card>
    </SettingsSection>
  </div>
</template>
