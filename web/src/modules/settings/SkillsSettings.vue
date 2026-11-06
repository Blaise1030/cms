<script setup lang="ts">
import { onMounted, ref } from "vue";
import { RotateCcw, Sparkles } from "@lucide/vue";
import { errorMessage } from "@orso/utils";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useDaemon } from "@/core/daemon";
import SettingsSection from "./SettingsSection.vue";

const daemon = useDaemon();
const path = ref("");
const defaultPath = ref("");
const loading = ref(true);
const saving = ref(false);
const saved = ref(false);
const error = ref<string | null>(null);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    const settings = await daemon.api.skillsSettings();
    path.value = settings.globalPath;
    defaultPath.value = settings.defaultGlobalPath;
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    loading.value = false;
  }
}

async function save() {
  saving.value = true;
  saved.value = false;
  error.value = null;
  try {
    const settings = await daemon.api.saveSkillsSettings({ globalPath: path.value });
    path.value = settings.globalPath;
    defaultPath.value = settings.defaultGlobalPath;
    saved.value = true;
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    saving.value = false;
  }
}

/** Editing the path makes the last save stale. */
function onInput(value: string | number) {
  path.value = String(value);
  saved.value = false;
}

/** The daemon resets to the default on an empty path; apply the returned default on success. */
async function reset() {
  saving.value = true;
  saved.value = false;
  error.value = null;
  try {
    const settings = await daemon.api.saveSkillsSettings({ globalPath: "" });
    path.value = settings.globalPath;
    defaultPath.value = settings.defaultGlobalPath;
    saved.value = true;
  } catch (err) {
    error.value = errorMessage(err);
  } finally {
    saving.value = false;
  }
}

onMounted(load);
</script>

<template>
  <SettingsSection label="Global skills path" :icon="Sparkles" data-testid="skills-settings">
    <Card class="gap-4 p-4">
      <p class="text-muted-foreground text-sm">
        Skills live in a shared folder, available to every project. Change where it lives below.
      </p>

      <div class="space-y-1.5">
        <Label for="skills-settings-path" class="text-xs font-medium">Global skills path</Label>
        <Input
          id="skills-settings-path"
          :model-value="path"
          data-testid="skills-settings-path"
          class="font-mono text-xs"
          :disabled="loading"
          @update:model-value="onInput"
        />
        <p class="text-muted-foreground text-xs">
          Default: <code>{{ defaultPath }}</code>
        </p>
      </div>

      <p v-if="error" role="alert" class="text-destructive text-xs" data-testid="skills-settings-error">
        {{ error }}
      </p>
      <p v-else-if="saved" role="status" class="text-muted-foreground text-xs" data-testid="skills-settings-saved">
        Saved.
      </p>

      <div class="flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          :disabled="loading || saving"
          data-testid="skills-settings-reset"
          @click="reset"
        >
          <RotateCcw aria-hidden="true" /> Reset to default
        </Button>
        <Button
          type="button"
          size="sm"
          :disabled="loading || saving"
          data-testid="skills-settings-save"
          @click="save"
        >
          {{ saving ? "Saving…" : "Save" }}
        </Button>
      </div>
    </Card>
  </SettingsSection>
</template>
