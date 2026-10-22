import { mount } from "svelte";
import App from "./App.svelte";
import "morphe/theme.css";
import { initTheme, themeStorageKey } from "morphe/theme";
import { installMenuFallback } from "morphe/menu.svelte";

themeStorageKey("kenon.theme");
initTheme();
installMenuFallback();

const target = document.getElementById("app");
if (!target) throw new Error("no #app element");

export default mount(App, { target });
