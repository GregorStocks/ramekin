(function () {
  console.log("[Ramekin] Bookmarklet started");
  var h = document.documentElement.outerHTML;
  var u = location.href;
  var o = "__ORIGIN__";
  console.log("[Ramekin] Origin:", o, "URL:", u, "HTML length:", h.length);

  if (document.getElementById("ramekin-capture-container")) {
    console.log("[Ramekin] Container already exists, exiting");
    return;
  }

  var c = document.createElement("div");
  c.id = "ramekin-capture-container";
  c.style.cssText =
    "position:fixed;bottom:20px;right:20px;width:320px;height:220px;z-index:2147483647;border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,0.4);overflow:hidden;";

  var f = document.createElement("iframe");
  f.src = o + "/capture";
  console.log("[Ramekin] Loading iframe:", f.src);
  f.style.cssText = "width:100%;height:100%;border:none;";
  f.onerror = function (e) {
    console.error("[Ramekin] Iframe load error:", e);
  };
  f.onload = function () {
    console.log("[Ramekin] Iframe loaded");
  };

  c.appendChild(f);
  document.body.appendChild(c);
  console.log("[Ramekin] Container added to page, waiting for ready message");

  window.addEventListener("message", function handler(e) {
    console.log(
      "[Ramekin] Message received, origin:",
      e.origin,
      "data:",
      typeof e.data === "string" ? e.data : e.data?.type,
    );
    if (e.origin !== o) {
      console.log("[Ramekin] Ignoring message from wrong origin");
      return;
    }
    if (e.data === "ready") {
      console.log("[Ramekin] Got ready, sending HTML");
      f.contentWindow.postMessage({ type: "html", html: h, url: u }, o);
      console.log("[Ramekin] HTML sent");
    } else if (e.data && e.data.type === "close") {
      console.log("[Ramekin] Close requested");
      c.remove();
      window.removeEventListener("message", handler);
    } else if (e.data && e.data.type === "viewRecipe") {
      console.log("[Ramekin] Opening recipe:", e.data.url);
      window.open(e.data.url, "_blank");
    }
  });
})();
