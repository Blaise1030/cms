import { toast } from "vue-sonner";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

/** Branch work goes through the agent, which follows the daemon's git guide for this app. */
function ask(text: string, sent: string) {
  useAgentOutboxStore().send({ kind: "text", text, submit: true });
  toast.info(sent);
}

export function askAgentForNewChange(title: string) {
  ask(`Start a new change called "${title.trim()}".`, `Asked the agent to start "${title.trim()}"`);
}
