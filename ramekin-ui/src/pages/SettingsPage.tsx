import { createSignal, createEffect, Show, Switch, Match } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";

type ConnectionStatus = "checking" | "connected" | "error";

export default function SettingsPage() {
  usePageTitle(() => "Settings");
  const { getUsersApi, setToken } = useAuth();

  const [username, setUsername] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<ConnectionStatus>("checking");
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [showLogoutConfirm, setShowLogoutConfirm] = createSignal(false);

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
