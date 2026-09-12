"use strict";
(() => {
  const invitation = location.href;
  const valid = /^#v2\.[A-Za-z0-9_-]{43}$/.test(location.hash) && !location.search;
  const status = document.getElementById("status");
  if (!valid) { status.textContent = "This invitation is incomplete. Ask the sender for a new link."; return; }
  const open = document.getElementById("open");
  // The fragment stays on this device; no analytics, fetch, or third-party script.
  open.href = "wisp-invite:v2." + btoa(invitation).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  open.hidden = false;
  const copy = document.getElementById("copy");
  copy.hidden = false;
  copy.addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(invitation); copy.textContent = "Copied!"; }
    catch (_) { status.textContent = "Copy the complete invitation from your address bar, then paste it into Wisp."; }
  });
})();
