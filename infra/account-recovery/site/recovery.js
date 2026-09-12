"use strict";
(() => {
  const byId = id => document.getElementById(id);
  const mode = location.pathname.replace(/\/$/, "").split("/").pop();
  let secureReset = false;
  let token = new URLSearchParams(location.hash.slice(1)).get("token") || "";
  // Fragment secrets never reach the HTTP server, referrers, or later history.
  history.replaceState(null, "", location.pathname);
  const showError = text => { byId("error-text").textContent = text; byId("error").hidden = false; };
  byId("dismiss").addEventListener("click", () => { byId("error").hidden = true; byId("error-text").textContent = ""; });
  const known = {
    secure_reset_required:"Open Wisp to reset this account’s secure password.",
    invalid_token:"This link is invalid or expired. Request a new one.",
    invalid_password:"Use a password of at least 12 characters and at most 1024 bytes.",
    recovery_email_unavailable:"That email address is unavailable for this account.",
    recovery_rate_limited:"Too many attempts. Please wait before trying again.",
    mail_unavailable:"Recovery email is temporarily unavailable. Please try again later."
  };
  async function post(path, body) {
    const abort = new AbortController();
    const timer = setTimeout(() => abort.abort(), 15000);
    try {
      const response = await fetch(path, {method:"POST",credentials:"omit",cache:"no-store",redirect:"error",referrerPolicy:"no-referrer",headers:{"Content-Type":"application/json"},body:JSON.stringify(body),signal:abort.signal});
      let result = {};
      const reader = response.body?.getReader();
      if (reader) {
        let bytes = new Uint8Array(0);
        while (true) {
          const {value,done} = await reader.read();
          if (done) break;
          if (bytes.length + value.length > 8192) { await reader.cancel(); throw new Error("Unrecognized response"); }
          const next = new Uint8Array(bytes.length+value.length);next.set(bytes);next.set(value,bytes.length);bytes=next;
        }
        try { result = JSON.parse(new TextDecoder().decode(bytes)); } catch (_) { throw new Error("Unrecognized response"); }
        if (!result || typeof result !== "object" || Array.isArray(result)) throw new Error("Unrecognized response");
      }
      if (!response.ok) {
        // Never display arbitrary server/proxy bodies, which might echo secrets.
        const error = new Error(known[result.code] || (response.status === 429 ? known.recovery_rate_limited : "Couldn't complete this request. Please try again later."));
        error.secureReset = result.code === "secure_reset_required";
        throw error;
      }
      return result;
    } catch (error) {
      if (Object.values(known).includes(error.message) || error.message === "Couldn't complete this request. Please try again later.") throw error;
      throw new Error(mode === "reset-password" ? "Couldn't confirm the result. Try signing in before requesting another reset link." : "Couldn't reach Wisp. Check your connection and try again.");
    } finally { clearTimeout(timer); }
  }
  function showNativeReset() {
    secureReset = true;
    byId("title").textContent = "Reset your secure password";
    byId("intro").textContent = "Finish in Wisp to keep secure sign-in on your device.";
    byId("form").hidden = true;byId("password-fields").hidden = true;
    byId("password").value = "";byId("confirm").value = "";
    byId("password").required = false;byId("confirm").required = false;
    byId("native-reset").hidden = false;byId("retry").hidden = false;
  }
  const setup = async () => {
    if (mode === "forgot-password") {
      token = "";byId("title").textContent="Forgot password?";
      byId("intro").textContent="We'll email a reset link if this account has a verified recovery address.";
      byId("identity-fields").hidden=false;byId("identifier").required=true;byId("submit").textContent="Send reset link";
    } else if (mode === "verify-email" || mode === "reset-password") {
      byId("title").textContent=mode === "verify-email" ? "Verify recovery email" : "Choose a new password";
      byId("intro").textContent=mode === "verify-email" ? "Confirm that this email address belongs to you." : "Use a strong password you haven't used elsewhere.";
      if (!/^[A-Za-z0-9_-]{43}$/.test(token)) throw new Error(known.invalid_token);
      if (mode === "reset-password") {
        byId("retry").hidden=false;
        const inspected = await post("/v2/accounts/password-reset/inspect", {token});
        if (inspected.valid !== true || typeof inspected.secure_reset_required !== "boolean") throw new Error("Couldn't complete this request. Please try again later.");
        if (inspected.secure_reset_required) { showNativeReset(); return; }
        byId("password-fields").hidden=false;byId("password").required=true;byId("confirm").required=true;
      }
      byId("submit").textContent=mode === "verify-email" ? "Verify email" : "Reset password";
    } else throw new Error("This recovery page is unavailable.");
    byId("form").hidden=false;
  };
  byId("form").addEventListener("submit", async event => {
    event.preventDefault();byId("error").hidden=true;
    if (secureReset) return;
    if (mode === "reset-password" && byId("password").value !== byId("confirm").value) { showError("Passwords do not match."); return; }
    byId("submit").disabled=true;
    try {
      if (mode === "forgot-password") await post("/v2/accounts/password-reset/request",{identifier:byId("identifier").value.trim()});
      else if (mode === "verify-email") await post("/v2/accounts/recovery-email/verify",{token});
      else await post("/v2/accounts/password-reset/complete",{token,new_password:byId("password").value});
      token="";byId("password").value="";byId("confirm").value="";
      byId("form").hidden=true;byId("retry").hidden=true;byId("success").hidden=false;
      byId("success").textContent=mode === "forgot-password" ? "If the account has a verified recovery email, a reset link is on its way. Check your inbox and spam folder. The link expires in 20 minutes." : mode === "verify-email" ? "Email verified. Return to Wisp and refresh your profile settings." : "Password changed. You can now sign in to Wisp with your new password.";
    } catch(error) { if (error.secureReset) showNativeReset();showError(error.message); }
    finally { byId("submit").disabled=false; }
  });
  setup().catch(error => {byId("intro").textContent="";showError(error.message);byId("retry").hidden=mode !== "reset-password";});
})();
