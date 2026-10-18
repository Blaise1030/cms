import { createPinia } from "pinia";
import { createApp } from "vue";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./index.css";
import App from "./App.vue";
import { useDaemon } from "./core/daemon";
import { createAppRouter } from "./router";

// Resolve the daemon config first: it removes the one-time `?t=` token from the URL, and the
// router must not pick that token up as part of its initial route.
useDaemon();

createApp(App).use(createPinia()).use(createAppRouter()).mount("#app");
