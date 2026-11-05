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
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  <div class="flex flex-col gap-4" data-testid="github-settings">
    <Card>
      <CardHeader>
        <CardTitle>GitHub</CardTitle>
        <CardDescription>
          Git and the GitHub tools are optional for local work. Sign in to share previews and
          publish.
        </CardDescription>
        <CardAction>
          <Button
            variant="outline"
            size="sm"
            :disabled="checking"
            data-testid="github-settings-recheck"
            @click="recheck"
          >
            <RefreshCw :class="{ 'animate-spin': checking }" aria-hidden="true" /> Recheck
          </Button>
        </CardAction>
      </CardHeader>

      <CardContent>
        <p
          v-if="loading"
          class="text-muted-foreground flex items-center gap-2 text-sm"
          data-testid="github-settings-loading"
        >
          <Spinner class="size-3.5" aria-hidden="true" /> Checking your setup…
        </p>

        <Alert v-else-if="failed || !status" variant="destructive" data-testid="github-settings-error">
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

        <ItemGroup v-else class="bg-card divide-y overflow-hidden rounded-lg border">
          <Item class="items-start px-4 py-3.5" data-testid="github-settings-git">
            <ItemMedia class="mt-0.5">
              <CircleCheck
                v-if="status.git"
                class="size-4 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-4" aria-hidden="true" />
            </ItemMedia>
            <ItemContent class="gap-1">
              <ItemTitle><GitBranch class="size-4" aria-hidden="true" /> Git</ItemTitle>
              <ItemDescription>Keeps the history of every change to your sites.</ItemDescription>
              <a
                v-if="!status.git"
                class="inline-flex items-center gap-1 self-start text-sm underline"
                href="https://git-scm.com/downloads/mac"
                target="_blank"
                rel="noopener noreferrer"
              >
                Get Git <ExternalLink class="size-3" aria-hidden="true" />
              </a>
            </ItemContent>
            <ItemActions class="text-muted-foreground text-xs">
              {{ status.git ? "Ready" : "Optional" }}
            </ItemActions>
          </Item>

          <Item class="items-start px-4 py-3.5" data-testid="github-settings-tools">
            <ItemMedia class="mt-0.5">
              <CircleCheck
                v-if="status.gh"
                class="size-4 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-4" aria-hidden="true" />
            </ItemMedia>
            <ItemContent class="gap-1">
              <ItemTitle><GitPullRequest class="size-4" aria-hidden="true" /> GitHub tools</ItemTitle>
              <ItemDescription>Connects your sites to GitHub.</ItemDescription>
              <a
                v-if="!status.gh"
                class="inline-flex items-center gap-1 self-start text-sm underline"
                href="https://cli.github.com/"
                target="_blank"
                rel="noopener noreferrer"
              >
                Get the GitHub tools <ExternalLink class="size-3" aria-hidden="true" />
              </a>
            </ItemContent>
            <ItemActions class="text-muted-foreground text-xs">
              {{ status.gh ? "Ready" : "Optional" }}
            </ItemActions>
          </Item>

          <Item class="items-start px-4 py-3.5" data-testid="github-settings-account">
            <ItemMedia class="mt-0.5">
              <CircleCheck
                v-if="githubSignedIn"
                class="size-4 text-emerald-600 dark:text-emerald-400"
                aria-hidden="true"
              />
              <CircleAlert v-else class="text-muted-foreground size-4" aria-hidden="true" />
            </ItemMedia>
            <ItemContent class="gap-1">
              <ItemTitle><UserRound class="size-4" aria-hidden="true" /> GitHub account</ItemTitle>
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
            <ItemActions class="text-muted-foreground text-xs">
              {{ githubSignedIn ? "Ready" : "Optional" }}
            </ItemActions>
          </Item>
        </ItemGroup>
      </CardContent>
    </Card>

    <DeviceCodeDialog />
  </div>
</template>
