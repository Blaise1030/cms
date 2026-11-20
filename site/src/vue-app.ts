import type { App } from "vue";
import ui from "@nuxt/ui/vue-plugin";

/** Runs for every Vue island and static Vue render: installs Nuxt UI. */
export default (app: App) => {
  app.use(ui);
};
