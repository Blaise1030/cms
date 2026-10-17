import { toast } from "vue-sonner";
import { useAgentOutboxStore } from "@/modules/chat/outbox";

/** The site's `@theme` follows DESIGN.md through the agent, not through the daemon. */
export function askAgentToApplyDesign() {
  useAgentOutboxStore().send({
    kind: "text",
    text: "Update the @theme block in src/styles/global.css (fonts, colours, radius, spacing) to match DESIGN.md, and follow its guidance in the components.",
    submit: true,
  });
  toast.info("Asked the agent to apply the design");
}
