import { extension, send, settings } from "./handoff.js";

const menus = new Map([
  ["page", { title: "Open Page in Riina", field: "pageUrl" }],
  ["link", { title: "Open Link in Riina", field: "linkUrl" }],
  ["video", { title: "Open Video in Riina", field: "srcUrl" }],
  ["audio", { title: "Open Audio in Riina", field: "srcUrl" }],
]);
const toolbarTitle = "Open in Riina";

async function updatePopup() {
  const { toolbar } = await settings();
  await extension.action.setPopup({ popup: toolbar === "menu" ? "popup.html" : "" });
}

async function registerMenus() {
  await extension.contextMenus.removeAll();
  for (const [context, { title }] of menus) {
    extension.contextMenus.create({ id: context, title, contexts: [context] });
  }
}

async function updateBadge(tabId, message) {
  if (tabId == null) return;
  await Promise.allSettled([
    extension.action.setBadgeText({ tabId, text: message ? "!" : "" }),
    extension.action.setTitle({ tabId, title: message || toolbarTitle }),
  ]);
}

async function open(tab, source, useDefault) {
  try {
    const mode = useDefault ? (await settings()).mode : "open";
    await send(tab, source, mode);
    await updateBadge(tab?.id);
  } catch (error) {
    await updateBadge(tab?.id, error.message);
  }
}

function reportSetupError(error) {
  console.error("Could not configure Open in Riina", error);
}

extension.runtime.onInstalled.addListener(() => {
  void Promise.all([registerMenus(), updatePopup()]).catch(reportSetupError);
});
extension.runtime.onStartup.addListener(() => {
  void updatePopup().catch(reportSetupError);
});
extension.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && changes.toolbar) void updatePopup().catch(reportSetupError);
});
extension.action.onClicked.addListener((tab) => {
  void open(tab, tab.url, true);
});
extension.contextMenus.onClicked.addListener((info, tab) => {
  const menu = menus.get(info.menuItemId);
  if (menu) void open(tab, info[menu.field], false);
});
