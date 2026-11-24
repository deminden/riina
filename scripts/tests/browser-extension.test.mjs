import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";

function event() {
  return { addListener(listener) { this.emit = listener; } };
}

const installed = event();
const started = event();
const toolbar = event();
const contextMenu = event();
const storageChanged = event();
const menus = new Map();
const sent = [];
const badges = [];
const titles = [];
let stored = {};
let popup;
let restricted = false;

globalThis.browser = {
  runtime: { onInstalled: installed, onStartup: started },
  storage: {
    onChanged: storageChanged,
    local: { get: async (defaults) => ({ ...defaults, ...stored }) },
  },
  action: {
    onClicked: toolbar,
    setPopup: async (details) => { popup = details.popup; },
    setBadgeText: async (details) => { badges.push(details); },
    setTitle: async (details) => { titles.push(details); },
  },
  contextMenus: {
    onClicked: contextMenu,
    removeAll: async () => { menus.clear(); },
    create: (details) => { menus.set(details.id, details); },
  },
  scripting: {
    executeScript: async (details) => {
      if (restricted) throw new Error("Missing host access");
      sent.push(details);
    },
  },
};

const directory = new URL("../../browser/open-in-riina/", import.meta.url);
const { linkFor, modes, settings } = await import(new URL("handoff.js", directory));
await import(new URL("background.js", directory));

test("browser manifests load the shared worker with only user-invoked permissions", async () => {
  const manifest = JSON.parse(await readFile(new URL("manifest.json", directory)));
  assert.equal(manifest.manifest_version, 3);
  assert.deepEqual(manifest.permissions, ["activeTab", "contextMenus", "scripting", "storage"]);
  assert.equal(manifest.host_permissions, undefined);
  assert.equal(manifest.content_scripts, undefined);
  assert.deepEqual(manifest.background.scripts, [manifest.background.service_worker]);
  assert.equal(manifest.background.type, "module");
  for (const file of [manifest.background.service_worker, manifest.options_ui.page, "popup.html"]) {
    await readFile(new URL(file, directory));
  }
});

test("all playback actions preserve the selected URL and encode one typed flag", () => {
  const source = "https://example.net/%2Fvideo.mkv?sig=A+B%3D&return=%2520#t=4";
  for (const [mode, flag] of [
    ["open", null], ["fullscreen", "full_screen"], ["pip", "pip"],
    ["enqueue", "enqueue"], ["window", "new_window"],
  ]) {
    const link = new URL(linkFor(source, mode));
    assert.equal(link.protocol, "riina:");
    assert.equal(link.host, "weblink");
    assert.deepEqual([...link.searchParams], flag ? [["url", source], [flag, "1"]] : [["url", source]]);
  }
  assert.equal(Object.keys(modes).length, 5);
  const local = "file:///C:/Media/M%C3%A9dia%20File.mkv";
  assert.equal(new URL(linkFor(local, "open")).searchParams.get("url"), local);
});

test("malformed targets, private schemes, credentials, and oversized payloads are refused", () => {
  for (const source of [
    undefined, "", "example.net", "javascript:alert(1)", "riina://open?url=x",
    "blob:https://example.net/video", "data:video/mp4,abc", "about:config",
    "https://alice:password@example.net/", "https://example.net/\nvideo",
    "file://server/video.mkv", "file:///video.mkv?secret=true", "file:///video.mkv#t=0",
  ]) {
    assert.throws(() => linkFor(source, "open"));
  }
  assert.throws(() => linkFor("https://example.net/", "__proto__"), /playback action/);
  assert.throws(() => linkFor("https://example.net/" + "é".repeat(3_000), "open"), /too long/);
});

test("installation, toolbar preferences, and four context actions use the current tab", async () => {
  stored = { toolbar: "invalid", mode: "__proto__" };
  assert.deepEqual(await settings(), { toolbar: "direct", mode: "open" });
  installed.emit();
  await setImmediate();
  assert.equal(popup, "");
  assert.deepEqual([...menus.values()].map((menu) => menu.contexts), [["page"], ["link"], ["video"], ["audio"]]);

  stored = { toolbar: "direct", mode: "pip" };
  const tab = { id: 84, url: "https://example.net/current" };
  toolbar.emit(tab);
  await setImmediate();
  assert.equal(new URL(sent.at(-1).args[0]).searchParams.get("pip"), "1");

  stored.toolbar = "menu";
  storageChanged.emit({ toolbar: {} }, "local");
  await setImmediate();
  assert.equal(popup, "popup.html");
  started.emit();
  await setImmediate();
  assert.equal(popup, "popup.html");

  const sources = {
    pageUrl: "https://example.net/frame",
    linkUrl: "https://example.net/link.mkv",
    srcUrl: "https://example.net/media.mkv",
  };
  for (const [menuItemId, field] of [
    ["page", "pageUrl"], ["link", "linkUrl"], ["video", "srcUrl"], ["audio", "srcUrl"],
  ]) {
    contextMenu.emit({ menuItemId, ...sources }, tab);
    await setImmediate();
    const { target, args, func } = sent.at(-1);
    assert.deepEqual(target, { tabId: tab.id });
    assert.deepEqual(args, [linkFor(sources[field], "open")]);
    const anchor = { click: () => assert.equal(anchor.href, args[0]) };
    globalThis.document = { createElementNS: () => anchor };
    try {
      func(args[0]);
    } finally {
      delete globalThis.document;
    }
  }

  restricted = true;
  toolbar.emit(tab);
  await setImmediate();
  assert.equal(badges.at(-1).text, "!");
  assert.match(titles.at(-1).title, /regular webpage/);
  restricted = false;
  toolbar.emit(tab);
  await setImmediate();
  assert.equal(badges.at(-1).text, "");
});
