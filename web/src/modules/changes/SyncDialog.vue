<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Questionnaire,
  QuestionnaireActions,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireInput,
  QuestionnaireItem,
  QuestionnaireNext,
  QuestionnairePrevious,
  QuestionnaireSubmit,
  QuestionnaireTitle,
} from "@/components/ui/questionnaire";
import { Spinner } from "@/components/ui/spinner";
import { useChangesStore } from "./store";

const props = defineProps<{ projectId: string }>();
const store = useChangesStore();

const state = computed(() => store.syncStatus?.state ?? "preparing");
const view = computed(() => {
  if (state.value === "done") return "done";
  if (state.value === "failed" || state.value === "idle") return "failed";
  if (state.value === "needs_input") return "question";
  return "working";
});
const closable = computed(() => view.value === "done" || view.value === "failed");
const questions = computed(() => store.syncStatus?.questions ?? []);

const confirmStop = ref(false);
const submitting = ref(false);
const picked = reactive<Record<string, string>>({});
const other = reactive<Record<string, string>>({});

const advancedError = ref<string | null>(null);

function clearAnswers() {
  for (const key of Object.keys(picked)) delete picked[key];
  for (const key of Object.keys(other)) delete other[key];
}

watch(() => questions.value.map((q) => q.id).join("\n"), clearAnswers);
watch(view, (_, previous) => {
  if (previous === "question") clearAnswers();
});
watch(closable, (isClosable) => {
  if (isClosable) confirmStop.value = false;
});

function pick(questionId: string, option: string) {
  picked[questionId] = option;
  other[questionId] = "";
}

function type(questionId: string, text: string) {
  other[questionId] = text;
  delete picked[questionId];
}

async function advanced() {
  advancedError.value = null;
  try {
    await store.openAdvanced(props.projectId);
  } catch {
    advancedError.value = "Couldn't open that view. Please try again.";
  }
}

function onEscape(event: KeyboardEvent) {
  if (closable.value) return;
  event.preventDefault();
  confirmStop.value = true;
}

async function stop() {
  confirmStop.value = false;
  await store.stopSync(props.projectId);
}

async function submitAnswers() {
  const answers = Object.fromEntries(
    questions.value.map((q) => [q.id, other[q.id]?.trim() || picked[q.id] || ""]),
  );
  submitting.value = true;
  try {
    await store.answerSync(props.projectId, answers);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <Dialog :open="true" @update:open="!$event && closable && store.closeSync()">
    <DialogContent
      :show-close-button="closable"
      class="sm:max-w-md"
      @pointer-down-outside.prevent
      @interact-outside.prevent
      @escape-key-down="onEscape"
    >
      <div v-if="view === 'working'" class="grid gap-4" data-testid="sync-working">
        <DialogHeader>
          <DialogTitle>Someone else also updated this site.</DialogTitle>
          <DialogDescription class="flex items-center gap-2">
            <Spinner />
            Combining your changes…
          </DialogDescription>
        </DialogHeader>
        <p class="text-muted-foreground text-sm" data-testid="sync-stage">{{ store.syncStatus?.stage ?? "" }}</p>
      </div>

      <div v-else-if="view === 'question'" class="grid gap-4" data-testid="sync-question">
        <DialogHeader>
          <DialogTitle>Someone else also updated this site.</DialogTitle>
          <DialogDescription>A quick question before your changes can be combined.</DialogDescription>
        </DialogHeader>
        <Questionnaire @submit.prevent="submitAnswers">
          <QuestionnaireItem v-for="q in questions" :key="q.id" :name="q.id" required>
            <QuestionnaireTitle>{{ q.text }}</QuestionnaireTitle>
            <QuestionnaireChoices>
              <QuestionnaireChoice
                v-for="option in q.options"
                :key="option"
                :value="option"
                :checked="picked[q.id] === option"
                @update:checked="$event && pick(q.id, option)"
              >
                {{ option }}
              </QuestionnaireChoice>
            </QuestionnaireChoices>
            <QuestionnaireInput
              :model-value="other[q.id] ?? ''"
              @update:model-value="type(q.id, $event)"
              data-testid="sync-other"
              placeholder="Something else…"
              aria-label="Something else"
            />
          </QuestionnaireItem>
          <QuestionnaireActions>
            <QuestionnairePrevious variant="outline">Back</QuestionnairePrevious>
            <QuestionnaireNext data-testid="sync-next">Next</QuestionnaireNext>
            <QuestionnaireSubmit data-testid="sync-continue" :disabled="submitting">Continue</QuestionnaireSubmit>
          </QuestionnaireActions>
        </Questionnaire>
      </div>

      <div v-else-if="view === 'done'" class="grid gap-4" data-testid="sync-done">
        <DialogHeader>
          <DialogTitle>Done. Your changes and the latest site are combined.</DialogTitle>
          <DialogDescription v-if="store.syncStatus?.summary">{{ store.syncStatus.summary }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button data-testid="sync-close" @click="store.closeSync()">Close</Button>
        </DialogFooter>
      </div>

      <div v-else class="grid gap-4" data-testid="sync-failed">
        <DialogHeader>
          <DialogTitle>Nothing changed. Your draft is safe.</DialogTitle>
        </DialogHeader>
        <p v-if="store.syncStatus?.error" class="text-destructive text-sm" role="alert">{{ store.syncStatus.error }}</p>
        <p v-if="advancedError" class="text-destructive text-sm" role="alert" data-testid="sync-advanced-error">{{ advancedError }}</p>
        <DialogFooter class="sm:justify-between">
          <Button variant="link" class="px-0" data-testid="sync-advanced" @click="advanced">I'll choose myself</Button>
          <div class="flex gap-2">
            <Button variant="outline" data-testid="sync-close" @click="store.closeSync()">Close</Button>
            <Button data-testid="sync-retry" @click="store.retrySync(projectId)">Try again</Button>
          </div>
        </DialogFooter>
      </div>

      <p
        v-if="view !== 'failed' && store.syncStatus?.error"
        class="text-destructive text-sm"
        role="alert"
      >
        {{ store.syncStatus.error }}
      </p>

      <template v-if="!closable">
        <div v-if="confirmStop" class="bg-muted grid gap-3 rounded-md p-3" data-testid="sync-stop-confirm">
          <p class="text-sm">Stop combining? Nothing will change.</p>
          <div class="flex justify-end gap-2">
            <Button variant="outline" size="sm" data-testid="sync-keep-going" @click="confirmStop = false">Keep going</Button>
            <Button variant="destructive" size="sm" data-testid="sync-confirm-stop" @click="stop">Stop</Button>
          </div>
        </div>
        <DialogFooter v-else>
          <Button variant="outline" data-testid="sync-stop" @click="confirmStop = true">Stop</Button>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>
</template>
