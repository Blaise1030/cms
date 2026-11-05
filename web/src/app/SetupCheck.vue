<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { CircleAlert, CircleCheck, ExternalLink, LogIn, RefreshCw } from "@lucide/vue";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { useDaemon } from "@/core/daemon";
import type { SetupStatus } from "@/core/types";
import AssistantSetup from "@/modules/assistant/AssistantSetup.vue";
import { useAssistantStore } from "@/modules/assistant/store";
import DeviceCodeDialog from "@/modules/github/DeviceCodeDialog.vue";
import { useGithubStore } from "@/modules/github/store";

/** `initial`: a status the caller already fetched; without it the check fetches its own. */
const props = defineProps<{ initial?: SetupStatus }>();
const emit = defineEmits<{ continue: [] }>();

const daemon = useDaemon();
const gh = useGithubStore();
const assistant = useAssistantStore();

const status = ref<SetupStatus | null>(props.initial ?? null);
const checking = ref(false);
const failed = ref(false);

/** Git, the GitHub CLI and the GitHub account stay system tools; the assistant is downloaded here. */
async function recheck() {
  checking.value = true;
  try {
    const [next] = await Promise.all([daemon.api.setupStatus(), assistant.load()]);
    status.value = next;
    failed.value = false;
  } catch {
    failed.value = true;
  } finally {
    checking.value = false;
  }
}

/** At least one assistant can start a chat right now, not merely be on the machine. */
const readyAssistants = computed(() => assistant.chatReadyProviders);
const assistantReady = computed(() => readyAssistants.value.length > 0);

const rows = computed(() => {
  const s = status.value;
  if (!s) return [];
  return [
    { key: "git", title: "Git", ready: s.git, readyText: "Git is installed. It keeps the history of every change to your sites." },
    { key: "gh", title: "GitHub CLI", ready: s.gh, readyText: "The GitHub CLI is installed." },
    { key: "github", title: "GitHub account", ready: s.githubSignedIn, readyText: "You're signed in to GitHub." },
  ];
});

// A finished in-app sign-in changes what the daemon reports.
watch(
  () => gh.user,
  (user) => {
    if (user) void recheck();
  },
);

onMounted(() => {
  if (!status.value) void recheck();
  gh.load().catch(() => {
    // Without GitHub status the sign-in row falls back to the CLI instructions.
  });
});
</script>

<template>
  <div class="flex h-full min-h-screen items-center justify-center p-6" data-testid="setup-check">
    <Card class="w-full max-w-lg">
      <CardHeader>
        <CardTitle>Let's get your Mac ready</CardTitle>
        <CardDescription>
          orso uses a few free tools to save, share and edit your sites. Install anything
          marked "Action needed", then select Recheck.
        </CardDescription>
      </CardHeader>
      <CardContent class="flex flex-col gap-4">
        <p v-if="failed" class="text-destructive text-sm" role="alert" data-testid="setup-error">
          We couldn't check your setup just now. Make sure orso is still running, then select
          Recheck.
        </p>
        <p v-else-if="!status" class="text-muted-foreground text-sm">Checking your setup…</p>
        <ul v-if="status" class="flex flex-col divide-y">
          <li v-for="row in rows" :key="row.key" class="flex gap-3 py-3" :data-testid="`setup-${row.key}`">
            <CircleCheck v-if="row.ready" class="mt-0.5 size-4 shrink-0 text-emerald-600" />
            <CircleAlert v-else class="text-destructive mt-0.5 size-4 shrink-0" />
            <div class="flex min-w-0 flex-1 flex-col gap-1.5 text-sm">
              <div class="flex items-center justify-between gap-2">
                <span class="font-medium">{{ row.title }}</span>
                <Badge :variant="row.ready ? 'secondary' : 'outline'">
                  {{ row.ready ? "Ready" : "Action needed" }}
                </Badge>
              </div>
              <p v-if="row.ready" class="text-muted-foreground">{{ row.readyText }}</p>
              <template v-else-if="row.key === 'git'">
                <p class="text-muted-foreground">
                  Git keeps the history of every change to your sites. Install it from the Git
                  website (it walks you through Apple's free developer tools).
                </p>
                <a class="inline-flex items-center gap-1 underline" href="https://git-scm.com/downloads/mac" target="_blank" rel="noopener noreferrer">
                  Get Git <ExternalLink class="size-3" />
                </a>
              </template>
              <template v-else-if="row.key === 'gh'">
                <p class="text-muted-foreground">The GitHub CLI connects your sites to GitHub so you can share changes.</p>
                <a class="inline-flex items-center gap-1 underline" href="https://cli.github.com/" target="_blank" rel="noopener noreferrer">
                  Get the GitHub CLI <ExternalLink class="size-3" />
                </a>
              </template>
              <template v-else-if="row.key === 'github'">
                <template v-if="gh.deviceFlow">
                  <p class="text-muted-foreground">Sign in so orso can open your sites and share your changes.</p>
                  <Button size="sm" variant="outline" class="self-start" data-testid="setup-sign-in" @click="gh.signIn()">
                    <LogIn /> Sign in with GitHub
                  </Button>
                  <p v-if="gh.error" class="text-destructive text-xs">GitHub sign-in didn't finish. Please try again.</p>
                </template>
                <p v-else-if="!status.gh" class="text-muted-foreground">Install the GitHub CLI first, then sign in with it.</p>
                <p v-else class="text-muted-foreground">
                  Open the Terminal app, type <code class="text-foreground">gh auth login</code>, press
                  Return and follow the steps. Then select Recheck.
                </p>
              </template>
            </div>
          </li>
        </ul>

        <section v-if="status" class="flex flex-col gap-3" data-testid="setup-assistant">
          <div class="flex items-center justify-between gap-2 text-sm">
            <span class="font-medium">Assistant</span>
            <Badge :variant="assistantReady ? 'secondary' : 'outline'">
              {{ assistantReady ? "Ready" : "Action needed" }}
            </Badge>
          </div>
          <p v-if="assistantReady" class="text-muted-foreground text-sm">
            {{ readyAssistants.map((p) => p.name).join(", ") }} ready to edit your sites.
          </p>
          <AssistantSetup
            v-if="!assistantReady || (assistant.node != null && assistant.siteBuilder.phase !== 'ready')"
            :autoload="!assistant.snapshot"
            @connect="emit('continue')"
          />
        </section>

        <p v-if="status" class="text-muted-foreground text-xs">
          Your sites are saved in <code class="text-foreground break-all">{{ status.projectsDir }}</code>.
        </p>
      </CardContent>
      <CardFooter class="flex justify-end gap-2">
        <Button variant="outline" :disabled="checking" data-testid="setup-recheck" @click="recheck">
          <RefreshCw :class="{ 'animate-spin': checking }" /> Recheck
        </Button>
        <Button data-testid="setup-continue" @click="emit('continue')">Continue</Button>
      </CardFooter>
    </Card>
    <DeviceCodeDialog />
  </div>
</template>
