import { extension, modes, send } from "./handoff.js";

const status = document.querySelector("#status");
const actions = document.querySelector("#actions");
const activeTab = extension.tabs.query({ active: true, currentWindow: true })
  .then(([tab]) => tab)
  .catch(() => undefined);

for (const [mode, label] of Object.entries(modes)) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      const tab = await activeTab;
      await send(tab, tab?.url, mode);
      window.close();
    } catch (error) {
      status.textContent = error.message;
      status.hidden = false;
    } finally {
      button.disabled = false;
    }
  });
  actions.append(button);
}

document.querySelector("#options").addEventListener("click", async () => {
  try {
    await extension.runtime.openOptionsPage();
    window.close();
  } catch {
    status.textContent = "Could not open Open in Riina settings.";
    status.hidden = false;
  }
});
