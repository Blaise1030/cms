<script setup lang="ts">
import { computed } from "vue";
import { ExternalLink, UserPlus } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { collaboratorsUrl } from "./invite";

const open = defineModel<boolean>("open", { default: false });
const props = defineProps<{ name: string; repo: string | null }>();

const url = computed(() => collaboratorsUrl(props.repo));
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <div class="grid gap-4">
        <DialogHeader>
          <DialogTitle>Invite a teammate</DialogTitle>
          <DialogDescription>Share {{ name }} in three steps.</DialogDescription>
        </DialogHeader>
        <ol class="grid list-decimal gap-2 pl-5 text-sm">
          <li>Add your teammate on GitHub</li>
          <li>They sign in with GitHub</li>
          <li>They open this site</li>
        </ol>
        <DialogFooter>
          <Button type="button" variant="outline" @click="open = false">Close</Button>
          <Button v-if="url" as-child>
            <a :href="url" target="_blank" rel="noopener noreferrer" data-testid="invite-collaborators-link">
              <UserPlus /> Add a teammate <ExternalLink class="opacity-60" />
            </a>
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>
</template>
