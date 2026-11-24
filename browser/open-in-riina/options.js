import { extension, modes, settings } from "./handoff.js";

const form = document.querySelector("form");
const toolbar = document.querySelector("#toolbar");
const mode = document.querySelector("#mode");
const save = form.querySelector("button");
const status = document.querySelector("#status");

for (const [value, label] of Object.entries(modes)) {
  mode.add(new Option(label, value));
}

toolbar.addEventListener("change", () => {
  mode.disabled = toolbar.value === "menu";
  status.textContent = "";
});
mode.addEventListener("change", () => {
  status.textContent = "";
});
form.addEventListener("submit", async (event) => {
  event.preventDefault();
  save.disabled = true;
  try {
    await extension.storage.local.set({ toolbar: toolbar.value, mode: mode.value });
    status.textContent = "Saved.";
  } catch {
    status.textContent = "Could not save these settings.";
  } finally {
    save.disabled = false;
  }
});

try {
  const saved = await settings();
  toolbar.value = saved.toolbar;
  mode.value = saved.mode;
  toolbar.disabled = false;
  mode.disabled = saved.toolbar === "menu";
  save.disabled = false;
} catch {
  status.textContent = "Could not load these settings.";
}
