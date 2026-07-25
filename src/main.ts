/**
 * Bootstrap dell'applicazione. Svelte 5: si monta con `mount`, non con
 * `new App({ target })`.
 */

import { mount } from "svelte";

import "./app.css";
import App from "./App.svelte";
import { collegaTema } from "./lib/tema.svelte";

// Il tema va agganciato prima del montaggio, così `data-tema` è già sul
// documento al primo disegno e non si vede il lampo del tema sbagliato.
collegaTema();

const radice = document.getElementById("app");
if (!radice) {
  throw new Error("manca l'elemento #app in index.html");
}

const app = mount(App, { target: radice });

export default app;
