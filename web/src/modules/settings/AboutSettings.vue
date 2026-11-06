<script setup lang="ts">
// App settings → About. Small and factual: the running version, where the data
// lives, and a way out to the project.
import { onMounted, ref } from "vue";
import { ExternalLink, HardDrive, Info } from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";
import { useDaemon } from "@/core/daemon";
import SettingsSection from "./SettingsSection.vue";

const REPO_URL = "https://github.com/Blaise1030/cms";
const DOCS_URL = `${REPO_URL}/tree/main/docs`;

const daemon = useDaemon();
const version = ref<string | null>(null);
const unavailable = ref(false);

onMounted(async () => {
  try {
    const health = await daemon.api.health();
    version.value = health.version;
  } catch {
    unavailable.value = true;
  }
});
</script>

<template>
  <SettingsSection label="About" :icon="Info" data-testid="about-settings">
    <Card class="gap-0 overflow-hidden p-0">
      <ItemGroup class="divide-y divide-border">
        <Item>
          <ItemMedia variant="icon">
            <Info aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>Version</ItemTitle>
            <ItemDescription>The version of orso running on this Mac.</ItemDescription>
          </ItemContent>
          <ItemActions>
            <Badge variant="secondary" data-testid="about-version">
              {{ version ? `v${version.replace(/^v/, "")}` : unavailable ? "Unknown" : "Checking…" }}
            </Badge>
          </ItemActions>
        </Item>

        <Item>
          <ItemMedia variant="icon">
            <HardDrive aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>Your data stays local</ItemTitle>
            <ItemDescription>
              Sites, content, and history live on this Mac. Nothing is uploaded unless you publish
              or share.
            </ItemDescription>
          </ItemContent>
        </Item>

        <Item>
          <ItemMedia variant="icon">
            <ExternalLink aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>Links</ItemTitle>
            <ItemDescription>
              <a :href="DOCS_URL" target="_blank" rel="noopener noreferrer">Documentation</a>
              and
              <a :href="REPO_URL" target="_blank" rel="noopener noreferrer">orso on GitHub</a>.
            </ItemDescription>
          </ItemContent>
        </Item>
      </ItemGroup>
    </Card>
  </SettingsSection>
</template>
