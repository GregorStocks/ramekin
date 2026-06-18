(function () {
  // Find our script tag to get the token and API URL
  var scripts = document.getElementsByTagName("script");
  var thisScript = scripts[scripts.length - 1];
  var src = thisScript.src;
  var params = new URL(src).searchParams;
  var token = params.get("token");
  var origin = new URL(src).origin;
  var apiOrigin = decodeURIComponent(params.get("api") || origin);
  var externalOrigin = decodeURIComponent(params.get("external") || origin);

  // --- Diagnostics: timestamped logger -------------------------------------
  var t0 = Date.now();
  function elapsedMs() { return Date.now() - t0; }
  function elapsedSecs() { return Math.round(elapsedMs() / 1000); }
  function ts() { return "[Ramekin +" + elapsedMs() + "ms] "; }
  function log(m) { console.log(ts() + m); }
  function warn(m) { console.warn(ts() + m); }
  function err(m) { console.error(ts() + m); }

  // --- Tunable timeouts (overridable via bookmarklet query params) ---------
  function intParam(name, dflt) {
    var raw = params.get(name);
    if (raw === null) return dflt;
    var n = parseInt(raw, 10);
    return Number.isFinite(n) && n >= 0 ? n : dflt;
  }
  var CAPTURE_TIMEOUT_MS = intParam("captureTimeout", 30000);
  var POLL_TIMEOUT_MS = intParam("pollTimeout", 10000);
  var STALL_WARN_MS = intParam("stallWarn", 60000);
  var POLL_INTERVAL_MS = intParam("pollInterval", 500);

  function fmtBytes(n) {
    if (n < 1024) return n + " B";
    if (n < 1024 * 1024) return (n / 1024).toFixed(0) + " KB";
    return (n / (1024 * 1024)).toFixed(1) + " MB";
  }

  if (!token) {
    err("No token in bookmarklet URL");
    alert("Ramekin: Invalid bookmarklet. Please get a new one from your Ramekin account.");
    return;
  }

  // Don't run twice
  if (document.getElementById("ramekin-capture-overlay")) {
    return;
  }
  log("init, token ok; api=" + apiOrigin);

  // Remove our own <script> tag before snapshotting: its src carries the
  // bookmarklet token, and the captured HTML is persisted server-side as the
  // fetch_html output. Leaving it in would store a reusable credential.
  if (thisScript && thisScript.parentNode) {
    thisScript.parentNode.removeChild(thisScript);
  }

  // Capture HTML before we add our overlay
  var html = document.documentElement.outerHTML;
  var url = location.href;
  log("captured HTML " + fmtBytes(html.length));

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

  var messageEl = document.getElementById("ramekin-message");
  var spinnerEl = document.getElementById("ramekin-spinner");
  var actionsEl = document.getElementById("ramekin-actions");

  // --- Overlay status helpers ---------------------------------------------
  var currentPhase = "Saving recipe...";
  var stalled = false;

  // Live elapsed-seconds counter on the spinner line.
  var timerId = setInterval(function () {
    if (stalled) return;
    if (spinnerEl.style.display !== "none") {
      messageEl.textContent = currentPhase + " (" + elapsedSecs() + "s)";
    }
  }, 1000);
  function stopTimer() {
    if (timerId) { clearInterval(timerId); timerId = null; }
  }

  function setPhase(text) {
    if (stalled) return;
    currentPhase = text;
    messageEl.textContent = text + " (" + elapsedSecs() + "s)";
  }

  function setStatus(message, isError, isDone) {
    stopTimer();
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
      '<a href="', externalOrigin, '/recipes/', recipeId, '" target="_blank" ',
      'style="padding:8px 16px;background:#4a9eff;color:#fff;text-decoration:none;',
      'border-radius:6px;font-weight:500;">View Recipe</a>',
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function () {
      overlay.remove();
    };
  }

  function showCloseButton() {
    actionsEl.style.display = "flex";
    actionsEl.innerHTML = [
      '<button id="ramekin-close" style="padding:8px 16px;background:#e0e0e0;',
      'border:none;border-radius:6px;cursor:pointer;">Close</button>'
    ].join("");
    document.getElementById("ramekin-close").onclick = function () {
      overlay.remove();
    };
  }

  // --- Networking with per-request timeout --------------------------------
  function fetchWithTimeout(resource, options, timeoutMs, label) {
    var controller = new AbortController();
    var timer = setTimeout(function () {
      warn(label + " timed out after " + timeoutMs + "ms - aborting");
      controller.abort();
    }, timeoutMs);
    options = options || {};
    options.signal = controller.signal;
    return fetch(resource, options).finally(function () {
      clearTimeout(timer);
    });
  }

  function handleFatal(label, timeoutMessage, e) {
    if (e && e.name === "AbortError") {
      warn(label + " aborted after timeout");
      setStatus(timeoutMessage, true);
    } else {
      err(label + " error: " + (e && e.message ? e.message : e));
      err("API origin: " + apiOrigin + " - if this looks like CORS, check the network tab");
      setStatus((e && e.message) || "Failed to save recipe", true);
    }
    showCloseButton();
  }

  // --- Stall watchdog ------------------------------------------------------
  var stallId = null;
  function startStallWatchdog() {
    stallId = setTimeout(function () {
      stalled = true;
      stopTimer();
      warn("watchdog: still not finished after " + STALL_WARN_MS + "ms");
      messageEl.textContent =
        "Taking longer than expected (" + elapsedSecs() + "s) - check console/network";
      messageEl.style.color = "#d32f2f";
      showCloseButton();
    }, STALL_WARN_MS);
  }
  function clearStallWatchdog() {
    if (stallId) { clearTimeout(stallId); stallId = null; }
  }

  // --- Poll loop -----------------------------------------------------------
  function pollJob(jobId, attempt) {
    log("poll #" + attempt + "...");
    fetchWithTimeout(apiOrigin + "/api/scrape/" + jobId, {
      headers: { "Authorization": "Bearer " + token }
    }, POLL_TIMEOUT_MS, "poll #" + attempt)
    .then(function (r) { return r.json(); })
    .then(function (job) {
      log("poll #" + attempt + " -> " + job.status);
      if (job.status === "completed" && job.recipe_id) {
        clearStallWatchdog();
        log("completed, recipe=" + job.recipe_id);
        setStatus("Recipe saved!", false, true);
        showActions(job.recipe_id);
      } else if (job.status === "failed") {
        clearStallWatchdog();
        warn("failed: " + (job.error || "unknown"));
        setStatus(job.error || "Failed to extract recipe", true);
        showCloseButton();
      } else {
        if (job.status === "parsing") {
          setPhase("Extracting recipe...");
        } else if (job.status === "pending") {
          setPhase("Queued...");
        } else {
          setPhase("Processing (" + job.status + ")...");
        }
        setTimeout(function () { pollJob(jobId, attempt + 1); }, POLL_INTERVAL_MS);
      }
    })
    .catch(function (e) {
      clearStallWatchdog();
      handleFatal("poll", "Status check timed out - check console/network", e);
    });
  }

  // --- Upload the captured page, then poll for the result -----------------
  function startCapture() {
    setPhase("Uploading page (" + fmtBytes(html.length) + ")");
    log("POST /api/scrape/capture...");
    fetchWithTimeout(apiOrigin + "/api/scrape/capture", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": "Bearer " + token
      },
      body: JSON.stringify({ html: html, source_url: url })
    }, CAPTURE_TIMEOUT_MS, "capture POST")
    .then(function (r) {
      log("capture response " + r.status);
      if (!r.ok) {
        return r.json().then(function (body) {
          throw new Error(body.error || ("Request failed (" + r.status + ")"));
        });
      }
      return r.json();
    })
    .then(function (result) {
      log("capture ok, job=" + result.id);
      setPhase("Queued...");
      startStallWatchdog();
      pollJob(result.id, 1);
    })
    .catch(function (e) {
      handleFatal("capture", "Upload timed out - check console/network", e);
    });
  }

  // --- Fail fast on an expired/invalid token before the big upload ---------
  // If the token is stale the server rejects the capture POST early; when that
  // happens mid-upload through a buffering proxy the response can be dropped
  // and the bookmarklet hangs for the full timeout. A cheap auth check first
  // keeps the failure fast and names the real cause.
  setPhase("Checking login");
  log("pre-flight GET /api/users/me...");
  fetchWithTimeout(apiOrigin + "/api/users/me", {
    headers: { "Authorization": "Bearer " + token }
  }, POLL_TIMEOUT_MS, "auth check")
  .then(function (r) {
    log("auth check -> " + r.status);
    if (r.status === 401 || r.status === 403) {
      warn("auth check failed: " + r.status);
      setStatus("Bookmarklet expired - get a new one from your Ramekin account", true);
      showCloseButton();
      return;
    }
    if (!r.ok) {
      throw new Error("Login check failed (" + r.status + ")");
    }
    startCapture();
  })
  .catch(function (e) {
    handleFatal("auth check", "Login check timed out - check console/network", e);
  });
})();
