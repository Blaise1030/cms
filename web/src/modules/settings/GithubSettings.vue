<script setup lang="ts">
// App settings → GitHub. Git and the GitHub tools are optional for local work;
// signing in only matters when a site is shared or published.
import { computed, onMounted, ref } from "vue";
import {
  CircleAlert,
  CircleCheck,
  ExternalLink,
  GitBranch,
  GitPullRequest,
  LogIn,
  RefreshCw,
  UserRound,
} from "@lucide/vue";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { Spinner } from "@/components/ui/spinner";
import { useDaemon } from "@/core/daemon";
import type { SetupStatus } from "@/core/types";
import DeviceCodeDialog from "@/modules/github/DeviceCodeDialog.vue";
import { useGithubStore } from "@/modules/github/store";
import SettingsSection from "./SettingsSection.vue";

const daemon = useDaemon();
const gh = useGithubStore();

const status = ref<SetupStatus | null>(null);
const checking = ref(false);
const failed = ref(false);

const githubSignedIn = computed(() => gh.user != null || status.value?.githubSignedIn === true);
const loading = computed(() => !status.value && !failed.value);

async function recheck() {
  checking.value = true;
  try {
    const [next] = await Promise.all([
      daemon.api.setupStatus(),
      gh.load().catch(() => {
        // Without GitHub status the account row keeps the manual sign-in hint.
      }),
    ]);
    status.value = next;
    failed.value = false;
  } catch {
    failed.value = true;
  } finally {
    checking.value = false;
  }
}

onMounted(() => {
  if (!status.value) void recheck();
});
</script>

<template>
  <SettingsSection label="Account" :icon="UserRound" data-testid="github-settings">
    <template #actions>
      <Button
        variant="outline"
        size="sm"
        :disabled="checking"
        data-testid="github-settings-recheck"
        @click="recheck"
      >
        <RefreshCw :class="{ 'animate-spin': checking }" aria-hidden="true" /> Recheck
      </Button>
    </template>

    <Card class="gap-0 overflow-hidden p-0">
      <p
        v-if="loading"
        class="text-muted-foreground flex items-center gap-2 p-4 text-sm"
        data-testid="github-settings-loading"
      >
        <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
      </p>

      <Alert
        v-else-if="failed || !status"
        variant="destructive"
        class="m-4"
        data-testid="github-settings-error"
      >
        <CircleAlert aria-hidden="true" />
        <AlertDescription class="flex flex-col items-start gap-2">
          <p>We couldn't check GitHub just now. Make sure orso is still running, then check again.</p>
          <Button
            variant="outline"
            size="sm"
            :disabled="checking"
            data-testid="github-settings-retry"
            @click="recheck"
          >
            <RefreshCw :class="{ 'animate-spin': checking }" aria-hidden="true" /> Check again
          </Button>
        </AlertDescription>
      </Alert>

      <ItemGroup v-else class="divide-y divide-border">
        <Item data-testid="github-settings-git">
          <ItemMedia variant="icon">
            <GitBranch aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>
              <CircleCheck
                v-if="status.git"
                class="size-3.5 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-3.5" aria-hidden="true" />
              Git
            </ItemTitle>
            <ItemDescription>Keeps the history of every change to your sites.</ItemDescription>
            <a
              v-if="!status.git"
              class="text-foreground inline-flex items-center gap-1 self-start text-sm underline underline-offset-4"
              href="https://git-scm.com/downloads/mac"
              target="_blank"
              rel="noopener noreferrer"
            >
              Get Git <ExternalLink class="size-3" aria-hidden="true" />
            </a>
          </ItemContent>
          <ItemActions>
            <Badge :variant="status.git ? 'secondary' : 'outline'">
              {{ status.git ? "Ready" : "Optional" }}
            </Badge>
          </ItemActions>
        </Item>

        <Item data-testid="github-settings-tools">
          <ItemMedia variant="icon">
            <GitPullRequest aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>
              <CircleCheck
                v-if="status.gh"
                class="size-3.5 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-3.5" aria-hidden="true" />
              GitHub tools
            </ItemTitle>
            <ItemDescription>Connects your sites to GitHub.</ItemDescription>
            <a
              v-if="!status.gh"
              class="text-foreground inline-flex items-center gap-1 self-start text-sm underline underline-offset-4"
              href="https://cli.github.com/"
              target="_blank"
              rel="noopener noreferrer"
            >
              Get the GitHub tools <ExternalLink class="size-3" aria-hidden="true" />
            </a>
          </ItemContent>
          <ItemActions>
            <Badge :variant="status.gh ? 'secondary' : 'outline'">
              {{ status.gh ? "Ready" : "Optional" }}
            </Badge>
          </ItemActions>
        </Item>

        <Item data-testid="github-settings-account">
          <ItemMedia variant="icon">
            <UserRound aria-hidden="true" />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>
              <CircleCheck
                v-if="githubSignedIn"
                class="size-3.5 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-3.5" aria-hidden="true" />
              GitHub account
            </ItemTitle>
            <ItemDescription v-if="githubSignedIn">
              <template v-if="gh.user">Signed in as {{ gh.user.login }}. </template>
              <template v-else>You're signed in to GitHub. </template>
              Share previews and publish when you are ready.
            </ItemDescription>
            <template v-else>
              <ItemDescription>
                Sign in so orso can open your sites and share your changes.
              </ItemDescription>
              <Button
                v-if="status.gh && gh.deviceFlow"
                size="sm"
                variant="outline"
                class="self-start"
                data-testid="github-settings-sign-in"
                @click="gh.signIn()"
              >
                <LogIn aria-hidden="true" /> Sign in with GitHub
              </Button>
              <p v-else-if="status.gh" class="text-muted-foreground text-xs">
                Sign in with GitHub on this Mac, then select Recheck.
              </p>
              <p v-else class="text-muted-foreground text-xs">
                Install the GitHub tools first, then sign in.
              </p>
            </template>
            <p v-if="gh.error" class="text-destructive text-xs" role="alert">
              GitHub sign-in didn't finish. Please try again.
            </p>
          </ItemContent>
          <ItemActions>
            <Badge :variant="githubSignedIn ? 'secondary' : 'outline'">
              {{ githubSignedIn ? "Ready" : "Optional" }}
            </Badge>
          </ItemActions>
        </Item>
      </ItemGroup>
    </Card>

    <DeviceCodeDialog />
  </SettingsSection>
</template>
