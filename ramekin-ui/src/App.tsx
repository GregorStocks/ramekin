import { createSignal, createEffect, createRoot, Show } from "solid-js";
import "./App.css";
import { AuthApi, TestApi, Configuration } from "ramekin-client";

const publicApi = new TestApi(new Configuration({ basePath: "" }));

// Auth state - wrapped in createRoot since it's at module level
const { token, setToken, getTestApi } = createRoot(() => {
  const [token, setToken] = createSignal<string | null>(
    localStorage.getItem("token"),
  );

  // Update localStorage when token changes
  createEffect(() => {
    const t = token();
    if (t) {
      localStorage.setItem("token", t);
    } else {
      localStorage.removeItem("token");
    }
  });

  const getAuthedConfig = () =>
    new Configuration({
      basePath: "",
      accessToken: () => token() ?? "",
    });

  const getTestApi = () => new TestApi(getAuthedConfig());

  return { token, setToken, getTestApi };
});

function AuthForm() {
  const [isLogin, setIsLogin] = createSignal(true);
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const api = new AuthApi(new Configuration({ basePath: "" }));

      if (isLogin()) {
        const response = await api.login({
          loginRequest: { username: username(), password: password() },
        });
        setToken(response.token);
      } else {
        const response = await api.signup({
          signupRequest: { username: username(), password: password() },
        });
        setToken(response.token);
      }
    } catch (err) {
      if (err instanceof Response) {
        const body = await err.json();
        setError(body.error || "Request failed");
      } else {
        setError("Request failed");
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="auth-form">
      <h2>{isLogin() ? "Login" : "Sign Up"}</h2>
      <form onSubmit={handleSubmit}>
        <div class="form-group">
          <label for="username">Username</label>
          <input
            id="username"
            type="text"
            value={username()}
            onInput={(e) => setUsername(e.currentTarget.value)}
            required
          />
        </div>
        <div class="form-group">
          <label for="password">Password</label>
          <input
            id="password"
            type="password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
            required
          />
        </div>
        <Show when={error()}>
          <div class="error">{error()}</div>
        </Show>
        <button type="submit" disabled={loading()}>
          {loading() ? "Loading..." : isLogin() ? "Login" : "Sign Up"}
        </button>
      </form>
      <p class="toggle-auth">
        {isLogin() ? "Don't have an account? " : "Already have an account? "}
        <button
          type="button"
          class="link-button"
          onClick={() => setIsLogin(!isLogin())}
        >
          {isLogin() ? "Sign Up" : "Login"}
        </button>
      </p>
    </div>
  );
}

function Dashboard() {
  const [message, setMessage] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  const pingServer = async () => {
    setLoading(true);
    try {
      const data = await getTestApi().ping();
      setMessage(data.message);
    } catch (error) {
      console.error("Failed to ping server:", error);
      setMessage("Error: Failed to ping server");
    } finally {
      setLoading(false);
    }
  };

  const logout = () => {
    setToken(null);
  };

  return (
    <div class="dashboard">
      <div class="header">
        <h2>Dashboard</h2>
        <button onClick={logout} class="logout-button">
          Logout
        </button>
      </div>

      <button onClick={pingServer} disabled={loading()}>
        {loading() ? "Loading..." : "Ping Server (Authenticated)"}
      </button>

      <Show when={message()}>
        <div class="response">Response: {message()}</div>
      </Show>
    </div>
  );
}

function UnauthedPing() {
  const [message, setMessage] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  const pingServer = async () => {
    setLoading(true);
    try {
      const data = await publicApi.unauthedPing();
      setMessage(data.message);
    } catch (error) {
      console.error("Failed to ping server:", error);
      setMessage("Error: Failed to ping server");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="unauthed-ping">
      <button onClick={pingServer} disabled={loading()}>
        {loading() ? "Loading..." : "Ping Server (Unauthenticated)"}
      </button>
      <Show when={message()}>
        <div class="response">Response: {message()}</div>
      </Show>
    </div>
  );
}

function App() {
  return (
    <div class="app">
      <h1>Ramekin</h1>
      <Show when={token()} fallback={<AuthForm />}>
        <Dashboard />
      </Show>
      <hr />
      <UnauthedPing />
    </div>
  );
}

export default App;
