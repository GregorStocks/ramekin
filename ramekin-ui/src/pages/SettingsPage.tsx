import {
  createSignal,
  createEffect,
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
import { createApiResource } from "../utils/asyncState";
import { createRequestTracker } from "../utils/requestTracker";
import { nameStatusPollDelay } from "../utils/nameStatusPolling";

type ConnectionStatus =
  | { kind: "checking" }
  | { kind: "connected" }
  | { kind: "error"; message: string };
type UploadState =
  | { kind: "idle" }
  | { kind: "uploading" }
  | { kind: "done" }
  | { kind: "failed"; message: string };

export default function SettingsPage() {
  usePageTitle(() => "Settings");
  const { getUsersApi, getClientLogsApi, getIngredientNamesApi, setToken } =
    useAuth();

  const [username, setUsername] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<ConnectionStatus>({
    kind: "checking",
  });
  const [showLogoutConfirm, setShowLogoutConfirm] = createSignal(false);
  const [uploadState, setUploadState] = createSignal<UploadState>({
    kind: "idle",
  });

  // On web the server is simply the origin serving the app.
  const serverUrl = window.location.origin;

  // Only the latest check may report: an older one finishing late must not
  // overwrite a newer answer.
  const connectionChecks = createRequestTracker();
  const checkConnection = async () => {
    const check = connectionChecks.start();
    setStatus({ kind: "checking" });
    try {
      const response = await getUsersApi().me();
      if (!connectionChecks.isCurrent(check)) return;
      setUsername(response.username);
      setStatus({ kind: "connected" });
    } catch (err) {
      const message = await extractApiError(err, "Could not reach the server");
      if (!connectionChecks.isCurrent(check)) return;
      setStatus({ kind: "error", message });
    }
  };
  onCleanup(() => connectionChecks.invalidate());

  createEffect(() => {
    checkConnection();
  });

  const connectionError = () => {
    const current = status();
    return current.kind === "error" ? current.message : null;
  };
  const uploadError = () => {
    const current = uploadState();
    return current.kind === "failed" ? current.message : null;
  };

  const logout = () => {
    setShowLogoutConfirm(false);
    setToken(null);
  };

  const handleUploadLogs = async () => {
    if (uploadState().kind === "uploading") return;
    setUploadState({ kind: "uploading" });
    logger.log("Settings", "uploading debug logs");
    try {
      await getClientLogsApi().createClientLog({
        createClientLogRequest: {
          platform: "web",
          osInfo: navigator.userAgent,
          content: logger.dump(),
        },
      });
      setUploadState({ kind: "done" });
    } catch (err) {
      setUploadState({
        kind: "failed",
        message: await extractApiError(err, "Failed to upload logs"),
      });
    }
  };

  // Ingredient names the catalog doesn't know, resolved in the background.
  // Consecutive failed status requests, for the retry backoff.
  const [statusFailures, setStatusFailures] = createSignal(0);
  const nameStatus = createApiResource(async () => {
    try {
      const result = await getIngredientNamesApi().getIngredientNamesStatus();
      setStatusFailures(0);
      return result;
    } catch (err) {
      setStatusFailures((failures) => failures + 1);
      throw err;
    }
  }, "Could not load ingredient recognition status");
  const refetchNameStatus = () => void nameStatus.refetch();
  const [retrying, setRetrying] = createSignal(false);
  const [nameMessage, setNameMessage] = createSignal<string | null>(null);
  const [nameError, setNameError] = createSignal<string | null>(null);
  const failedCount = () => nameStatus.data()?.failed ?? 0;
  // Keep the counts current while names are waiting, and keep retrying a
  // failed status request with backoff.
  createEffect(() => {
    if (nameStatus.loading()) return;
    const delay = nameStatusPollDelay(
      nameStatus.error() ? statusFailures() : 0,
      nameStatus.data()?.pending ?? 0,
    );
    if (delay === null) return;
    const timer = setTimeout(refetchNameStatus, delay);
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
              <span class="settings-connection" data-status={status().kind}>
                <span class="settings-connection-dot" aria-hidden="true" />
                <Switch>
                  <Match when={status().kind === "checking"}>Checking…</Match>
                  <Match when={status().kind === "connected"}>Connected</Match>
                  <Match when={connectionError()}>
                    {(message) => message()}
                  </Match>
                </Switch>
              </span>
            </dd>
          </div>
        </dl>
        <button
          type="button"
          class="btn btn-small"
          onClick={checkConnection}
          disabled={status().kind === "checking"}
        >
          {status().kind === "checking" ? "Checking…" : "Check again"}
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
          calories. Until then they count as unknown in calorie estimates.
        </p>
        <Show when={nameStatus.error()}>
          {(error) => (
            <p class="error settings-name-unavailable">
              Recognition status is unavailable: {error()}
            </p>
          )}
        </Show>
        <Show when={nameStatus.data()}>
          {(status) => (
            <>
              <p class="settings-name-counts">
                {status().recognized} recognized · {status().estimated}{" "}
                estimated · {status().notFood} not food · {status().unknown}{" "}
                unknown · {status().pending} pending · {status().failed} failed
              </p>
              <Show when={status().stall}>
                {(stall) => (
                  <p class="error settings-name-stall" role="status">
                    Recognition is stalled: {stall().error}. It will try again
                    by {stall().nextAttemptAt.toLocaleTimeString()}.
                  </p>
                )}
              </Show>
              <Show when={status().failures.length > 0}>
                <ul class="settings-name-failures">
                  <For each={status().failures}>
                    {(failure) => (
                      <li>
                        {failure.name}: {failure.error}
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

      <section class="settings-section" aria-label="Re-parse recipes">
        <h3>Re-parse recipes</h3>
        <p>
          To read recipes again from the page or file they came from with the
          current parser, select them in the cookbook and choose Re-normalize…,
          then Re-extract from saved source.
        </p>
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
          disabled={uploadState().kind === "uploading"}
          onClick={handleUploadLogs}
        >
          {uploadState().kind === "uploading"
            ? "Uploading…"
            : "Upload debug logs"}
        </button>
        <Show when={uploadState().kind === "done"}>
          <p class="success">Logs uploaded.</p>
        </Show>
        <Show when={uploadError()}>
          {(message) => <p class="error">{message()}</p>}
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
