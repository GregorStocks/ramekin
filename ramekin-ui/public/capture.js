(function () {
  // Find our script tag to get the token and API URL
  var scripts = document.getElementsByTagName("script");
  var thisScript = scripts[scripts.length - 1];
  var src = thisScript.src;
  var params = new URL(src).searchParams;
  var token = params.get("token");
  var origin = new URL(src).origin;
  var apiOrigin = decodeURIComponent(params.get("api") || origin);

  console.log("[Ramekin] Bookmarklet loaded");
  console.log("[Ramekin] Script origin:", origin);
  console.log("[Ramekin] API origin:", apiOrigin);

  if (!token) {
    console.error("[Ramekin] No token in bookmarklet URL");
    alert("Ramekin: Invalid bookmarklet. Please get a new one from your Ramekin account.");
    return;
  }

  // Don't run twice
  if (document.getElementById("ramekin-capture-overlay")) {
    return;
  }

  // Capture HTML before we add our overlay
  var html = document.documentElement.outerHTML;
  var url = location.href;

  // Create overlay UI
  var overlay = document.createElement("div");
  overlay.id = "ramekin-capture-overlay";
  overlay.innerHTML = [
    '<div style="position:fixed;bottom:20px;right:20px;width:300px;padding:16px;',
    'background:#fff;border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,0.3);',
    'font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-size:14px;',
    'z-index:2147483647;color:#333;">',
    '<div id="ramekin-status" style="display:flex;align-items:center;gap:8px;">',
    '<div id="ramekin-spinner" style="width:20px;height:20px;border:2px solid #e0e0e0;',
    'border-top-color:#4a9eff;border-radius:50%;animation:ramekin-spin 0.8s linear infinite;"></div>',
    '<span id="ramekin-message">Saving recipe...</span>',
    '</div>',
    '<div id="ramekin-actions" style="display:none;margin-top:12px;display:flex;gap:8px;"></div>',
    '<style>@keyframes ramekin-spin{to{transform:rotate(360deg)}}</style>',
    '</div>'
  ].join("");
  document.body.appendChild(overlay);

  var statusEl = document.getElementById("ramekin-status");
  var messageEl = document.getElementById("ramekin-message");
  var spinnerEl = document.getElementById("ramekin-spinner");
  var actionsEl = document.getElementById("ramekin-actions");

  function setStatus(message, isError, isDone) {
    messageEl.textContent = message;
    if (isError) {
      messageEl.style.color = "#d32f2f";
    }
    if (isDone || isError) {
      spinnerEl.style.display = "none";
    }
  }

  function showActions(recipeId) {
    actionsEl.style.display = "flex";
    actionsEl.innerHTML = [
      '<a href="', origin, '/recipes/', recipeId, '" target="_blank" ',
      'style="padding:8px 16px;background:#4a9eff;color:#fff;text-decoration:none;',
      'border-radius:6px;font-weight:500;">View Recipe</a>',
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function() {
      overlay.remove();
    };
  }

  function showCloseButton() {
    actionsEl.style.display = "flex";
    actionsEl.innerHTML = [
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function() {
      overlay.remove();
    };
  }

  function pollJob(jobId) {
    fetch(apiOrigin + "/api/scrape/" + jobId, {
      headers: { "Authorization": "Bearer " + token }
    })
    .then(function(r) { return r.json(); })
    .then(function(job) {
      if (job.status === "completed" && job.recipe_id) {
        setStatus("Recipe saved!", false, true);
        showActions(job.recipe_id);
      } else if (job.status === "failed") {
        setStatus(job.error || "Failed to extract recipe", true);
        showCloseButton();
      } else {
        // Still processing
        var statusText = job.status === "parsing" ? "Extracting recipe..." : "Processing...";
        setStatus(statusText);
        setTimeout(function() { pollJob(jobId); }, 500);
      }
    })
    .catch(function(err) {
      console.error("[Ramekin] Poll error:", err);
      console.error("[Ramekin] API origin:", apiOrigin);
      console.error("[Ramekin] This may be a CORS issue - check network tab");
      setStatus("Error checking status", true);
      showCloseButton();
    });
  }

  // Start the capture
  fetch(apiOrigin + "/api/scrape/capture", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": "Bearer " + token
    },
    body: JSON.stringify({ html: html, source_url: url })
  })
  .then(function(r) {
    if (!r.ok) {
      return r.json().then(function(body) {
        throw new Error(body.error || "Request failed");
      });
    }
    return r.json();
  })
  .then(function(result) {
    pollJob(result.id);
  })
  .catch(function(err) {
    console.error("[Ramekin] Capture error:", err);
    console.error("[Ramekin] API origin:", apiOrigin);
    console.error("[Ramekin] This may be a CORS issue - check network tab");
    setStatus(err.message || "Failed to save recipe", true);
    showCloseButton();
  });
})();
