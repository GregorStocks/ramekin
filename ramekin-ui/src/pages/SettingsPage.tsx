import {
  createSignal,
  createEffect,
  createResource,
  onCleanup,
  For,
  Show,
  Switch,
  Match,
} from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";

type ConnectionStatus = "checking" | "connected" | "error";
type UploadState = "idle" | "uploading" | "done";

export default function SettingsPage() {
  usePageTitle(() => "Settings");
  const { getUsersApi, getClientLogsApi, getIngredientNamesApi, setToken } =
    useAuth();

  const [username, setUsername] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<ConnectionStatus>("checking");
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [showLogoutConfirm, setShowLogoutConfirm] = createSignal(false);
  const [uploadState, setUploadState] = createSignal<UploadState>("idle");
  const [uploadError, setUploadError] = createSignal<string | null>(null);

  // On web the server is simply the origin serving the app.
  const serverUrl = window.location.origin;

  const checkConnection = async () => {
    setStatus("checking");
    setErrorMessage(null);
    try {
      const response = await getUsersApi().me();
      setUsername(response.username);
      setStatus("connected");
    } catch (err) {
      setStatus("error");
      setErrorMessage(await extractApiError(err, "Could not reach the server"));
    }
  };

  createEffect(() => {
    checkConnection();
  });

  const logout = () => {
    setShowLogoutConfirm(false);
    setToken(null);
  };

  const handleUploadLogs = async () => {
    setUploadState("uploading");
    setUploadError(null);
    logger.log("Settings", "uploading debug logs");
    try {
      await getClientLogsApi().createClientLog({
        createClientLogRequest: {
          platform: "web",
          osInfo: navigator.userAgent,
          content: logger.dump(),
        },
      });
      setUploadState("done");
    } catch (err) {
      setUploadState("idle");
      setUploadError(await extractApiError(err, "Failed to upload logs"));
    }
  };

  // Ingredient names the catalog doesn't know, resolved in the background.
  const [nameStatus, { refetch: refetchNameStatus }] = createResource(() =>
    getIngredientNamesApi().getIngredientNamesStatus(),
  );
  const [retrying, setRetrying] = createSignal(false);
  const [nameMessage, setNameMessage] = createSignal<string | null>(null);
  const [nameError, setNameError] = createSignal<string | null>(null);
  const failedCount = () =>
    (nameStatus()?.failed ?? 0) + (nameStatus()?.weights.failed ?? 0);
  // Keep the counts current while names or weights are waiting.
  createEffect(() => {
    const status = nameStatus();
    if ((status?.pending ?? 0) + (status?.weights.pending ?? 0) === 0) return;
    const timer = setTimeout(() => refetchNameStatus(), 2000);
    onCleanup(() => clearTimeout(timer));
  });
  const handleRetryNames = async () => {
    setRetrying(true);
    setNameMessage(null);
    setNameError(null);
    try {
      const { queued } = await getIngredientNamesApi().retryIngredientNames();
      setNameMessage(`Queued ${queued} again.`);
      refetchNameStatus();
    } catch (err) {
      setNameError(await extractApiError(err, "Failed to queue names"));
    } finally {
      setRetrying(false);
    }
  };

  return (
    <div class="settings-page">
      <div class="page-header">
        <h2>Settings</h2>
      </div>

      <section class="settings-section">
        <h3>Account</h3>
        <dl class="settings-list">
          <div class="settings-row">
            <dt>Username</dt>
            <dd>
              <Show
                when={username()}
                fallback={<span class="settings-value-muted">—</span>}
              >
                {username()}
              </Show>
            </dd>
          </div>
          <div class="settings-row">
            <dt>Server</dt>
            <dd class="settings-value-mono">{serverUrl}</dd>
          </div>
          <div class="settings-row">
            <dt>Connection</dt>
            <dd>
              <span class="settings-connection" data-status={status()}>
                <span class="settings-connection-dot" aria-hidden="true" />
                <Switch>
                  <Match when={status() === "checking"}>Checking…</Match>
                  <Match when={status() === "connected"}>Connected</Match>
                  <Match when={status() === "error"}>{errorMessage()}</Match>
                </Switch>
              </span>
            </dd>
          </div>
        </dl>
        <button
          type="button"
          class="btn btn-small"
          onClick={checkConnection}
          disabled={status() === "checking"}
        >
          {status() === "checking" ? "Checking…" : "Check again"}
        </button>
      </section>

      <section class="settings-section">
        <h3>Library</h3>
        <A href="/tags" class="settings-link">
          Manage Tags
        </A>
      </section>

      <section class="settings-section" aria-label="Ingredient recognition">
        <h3>Ingredient recognition</h3>
        <p>
          Ingredient names the catalog doesn't know are identified in the
          background, a generic name ("cheese") gets the food a recipe most
          likely means, and a food the catalog has no entry for gets estimated
          calories. Weights it lacks for a food (a cup of capers, a bunch of
          kale) are estimated too. Until then they count as unknown in calorie
          estimates.
        </p>
        <Show when={nameStatus()}>
          {(status) => (
            <>
              <p class="settings-name-counts">
                {status().recognized} recognized · {status().estimated}{" "}
                estimated · {status().notFood} not food · {status().unknown}{" "}
                unknown · {status().pending} pending · {status().failed} failed
              </p>
              <p class="settings-weight-counts">
                Weights: {status().weights.estimated} estimated ·{" "}
                {status().weights.noTypicalWeight} no typical weight ·{" "}
                {status().weights.pending} pending · {status().weights.failed}{" "}
                failed
              </p>
              <Show
                when={
                  status().failures.length + status().weights.failures.length >
                  0
                }
              >
                <ul class="settings-name-failures">
                  <For each={status().failures}>
                    {(failure) => (
                      <li>
                        {failure.name}: {failure.error}
                      </li>
                    )}
                  </For>
                  <For each={status().weights.failures}>
                    {(failure) => (
                      <li>
                        {failure.unit} of {failure.food}: {failure.error}
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </>
          )}
        </Show>
        <button
          type="button"
          class="btn btn-small"
          disabled={retrying() || failedCount() === 0}
          onClick={handleRetryNames}
        >
          {retrying() ? "Retrying…" : "Retry failed"}
        </button>
        <Show when={nameMessage()}>
          <p class="success" role="status">
            {nameMessage()}
          </p>
        </Show>
        <Show when={nameError()}>
          <p class="error">{nameError()}</p>
        </Show>
      </section>

      <section class="settings-section">
        <h3>Diagnostics</h3>
        <p>
          Upload this session's debug logs to the server so performance problems
          can be investigated.
        </p>
        <button
          type="button"
          class="btn btn-small"
          disabled={uploadState() === "uploading"}
          onClick={handleUploadLogs}
        >
          {uploadState() === "uploading" ? "Uploading…" : "Upload debug logs"}
        </button>
        <Show when={uploadState() === "done"}>
          <p class="success">Logs uploaded.</p>
        </Show>
        <Show when={uploadError()}>
          <p class="error">{uploadError()}</p>
        </Show>
      </section>

      <section class="settings-section">
        <button
          type="button"
          class="btn btn-danger"
          onClick={() => setShowLogoutConfirm(true)}
        >
          Sign Out
        </button>
      </section>

      <Modal
        isOpen={showLogoutConfirm}
        onClose={() => setShowLogoutConfirm(false)}
        title="Sign out of Ramekin?"
        actions={
          <>
            <button
              type="button"
              class="btn"
              onClick={() => setShowLogoutConfirm(false)}
            >
              Cancel
            </button>
            <button type="button" class="btn btn-danger" onClick={logout}>
              Sign Out
            </button>
          </>
        }
      >
        <p>You'll need to sign in again to save recipes.</p>
      </Modal>
    </div>
  );
}
