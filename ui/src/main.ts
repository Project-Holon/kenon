import { mount } from "svelte";
import App from "./App.svelte";
import "./theme.css";
import { initTheme } from "./lib/theme";
import { installMenuFallback } from "./lib/menu.svelte";

initTheme();
installMenuFallback();

const target = document.getElementById("app");
if (!target) throw new Error("no #app element");

export default mount(App, { target });
