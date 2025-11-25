import { createSignal, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { AuthApi, Configuration } from "ramekin-client";
import { useAuth } from "../context/AuthContext";

export default function LoginPage() {
  const navigate = useNavigate();
  const { setToken } = useAuth();

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
      navigate("/");
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
    <div class="login-page">
      <h1>Ramekin</h1>
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
    </div>
  );
}
