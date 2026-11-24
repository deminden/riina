export const extension = globalThis.browser ?? globalThis.chrome;

export const modes = Object.freeze({
  open: "Open in Riina",
  fullscreen: "Open Fullscreen",
  pip: "Open in Picture-in-Picture",
  enqueue: "Add to Playlist",
  window: "Open in New Window",
});

const flags = new Map([
  ["fullscreen", "full_screen"],
  ["pip", "pip"],
  ["enqueue", "enqueue"],
  ["window", "new_window"],
]);

export const defaults = Object.freeze({ toolbar: "direct", mode: "open" });
const maxLinkBytes = 16 * 1024;

export function linkFor(source, mode) {
  if (typeof source !== "string" || /[\u0000-\u001f\u007f-\u009f]/u.test(source)) {
    throw new Error("Choose a webpage, link, or media file to open.");
  }

  const url = URL.canParse(source) ? new URL(source) : null;
  if (!url || !["http:", "https:", "file:"].includes(url.protocol) ||
      url.username || url.password ||
      (url.protocol === "file:" && (url.hostname || url.search || url.hash))) {
    throw new Error("Choose an HTTP or HTTPS address, or a local media file.");
  }
  if (!Object.hasOwn(modes, mode)) {
    throw new Error("Choose an available playback action.");
  }

  // Validate the parsed URL, but pass along the original spelling of signed links.
  const query = new URLSearchParams({ url: source });
  const flag = flags.get(mode);
  if (flag) query.set(flag, "1");
  const link = `riina://weblink?${query}`;
  if (link.length > maxLinkBytes) {
    throw new Error("This address is too long to send to Riina.");
  }
  return link;
}

export async function settings() {
  const saved = await extension.storage.local.get(defaults);
  return {
    toolbar: saved.toolbar === "menu" ? "menu" : "direct",
    mode: Object.hasOwn(modes, saved.mode) ? saved.mode : defaults.mode,
  };
}

// The browser serializes this function into the user-invoked tab.
function dispatchLink(link) {
  const anchor = document.createElementNS("http://www.w3.org/1999/xhtml", "a");
  anchor.href = link;
  anchor.click();
}

export async function send(tab, source, mode) {
  const link = linkFor(source, mode);
  if (tab?.id == null) throw new Error("Choose a browser tab to open in Riina.");
  try {
    await extension.scripting.executeScript({
      target: { tabId: tab.id },
      func: dispatchLink,
      args: [link],
    });
  } catch {
    throw new Error("Riina cannot open links from this tab. Try from a regular webpage.");
  }
}
