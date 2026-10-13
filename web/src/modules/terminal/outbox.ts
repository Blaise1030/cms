import { defineStore } from "pinia";
import { ref } from "vue";
import { elementPrompt, type PickedElement } from "@/modules/preview/elementPicker";

/** Something another panel wants to put in front of the agent. `submit` sends it as a request
 *  (starting a session if none is running) instead of leaving it in the input to edit. */
export type AgentMessage =
  | { kind: "text"; text: string; submit?: boolean }
  | { kind: "element"; element: PickedElement };

export function isRequest(message: AgentMessage): boolean {
  return message.kind === "text" && message.submit === true;
}

/** The text pasted into the agent's input for a message. */
export function agentPrompt(message: AgentMessage): string {
  return message.kind === "element" ? elementPrompt(message.element) : message.text;
}

/** Messages waiting for the agent terminal, held until its session can take input. */
export const useAgentOutboxStore = defineStore("agentOutbox", () => {
  const messages = ref<AgentMessage[]>([]);
  /** Bumped on every send, so the page can bring the agent into view even after a quick drain. */
  const sent = ref(0);

  function send(message: AgentMessage) {
    messages.value.push(message);
    sent.value++;
  }

  function drain(): AgentMessage[] {
    return messages.value.splice(0);
  }

  return { messages, sent, send, drain };
});
