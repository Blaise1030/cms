import { toast } from "vue-sonner";
import { useAgentOutboxStore } from "@/modules/terminal/outbox";

/** Branch work goes through the agent, which follows the daemon's git guide for this app. */
function ask(text: string, sent: string) {
  useAgentOutboxStore().send({ kind: "text", text, submit: true });
  toast.info(sent);
}

export function askAgentToGetLatest() {
  ask(
    "Bring the latest version of the site (main) into my current change.",
    "Asked the agent to get the latest",
  );
}

export function askAgentForNewChange(title: string) {
  ask(`Start a new change called "${title.trim()}".`, `Asked the agent to start "${title.trim()}"`);
}
