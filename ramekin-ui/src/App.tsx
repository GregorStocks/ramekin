import { createSignal, createEffect, createRoot, Show, For } from "solid-js";
import "./App.css";
import { AuthApi, TestingApi, PhotosApi, Configuration } from "ramekin-client";
import type { PhotoSummary } from "ramekin-client";

const publicApi = new TestingApi(new Configuration({ basePath: "" }));

// Auth state - wrapped in createRoot since it's at module level
const { token, setToken, getPhotosApi } = createRoot(() => {
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

  const getTestingApi = () => new TestingApi(getAuthedConfig());
  const getPhotosApi = () => new PhotosApi(getAuthedConfig());

  return { token, setToken, getTestingApi, getPhotosApi };
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

function PhotoUpload(props: { onUploadComplete: () => void }) {
  const [uploading, setUploading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [success, setSuccess] = createSignal(false);

  const handleFileSelect = async (e: Event) => {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    setUploading(true);
    setError(null);
    setSuccess(false);

    try {
      await getPhotosApi().upload({ file });
      setSuccess(true);
      input.value = "";
      props.onUploadComplete();
    } catch (err) {
      if (err instanceof Response) {
        const body = await err.json();
        setError(body.error || "Upload failed");
      } else {
        setError("Upload failed");
      }
    } finally {
      setUploading(false);
    }
  };

  return (
    <div class="photo-upload">
      <h3>Upload Photo</h3>
      <input
        type="file"
        accept="image/jpeg,image/png,image/gif,image/webp"
        onChange={handleFileSelect}
        disabled={uploading()}
      />
      <Show when={uploading()}>
        <p>Uploading...</p>
      </Show>
      <Show when={error()}>
        <p class="error">{error()}</p>
      </Show>
      <Show when={success()}>
        <p class="success">Photo uploaded successfully!</p>
      </Show>
    </div>
  );
}

function PhotoGallery() {
  const [photos, setPhotos] = createSignal<PhotoSummary[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  const loadPhotos = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getPhotosApi().listPhotos();
      setPhotos(response.photos);
    } catch (err) {
      setError("Failed to load photos");
    } finally {
      setLoading(false);
    }
  };

  // Load photos on mount
  loadPhotos();

  return (
    <div class="photo-gallery">
      <div class="gallery-header">
        <h3>Your Photos</h3>
        <button onClick={loadPhotos} disabled={loading()}>
          Refresh
        </button>
      </div>

      <PhotoUpload onUploadComplete={loadPhotos} />

      <Show when={loading()}>
        <p>Loading photos...</p>
      </Show>

      <Show when={error()}>
        <p class="error">{error()}</p>
      </Show>

      <Show when={!loading() && photos().length === 0}>
        <p>No photos yet. Upload your first photo!</p>
      </Show>

      <div class="photo-grid">
        <For each={photos()}>
          {(photo) => (
            <div class="photo-card">
              <img
                src={`data:image/jpeg;base64,${photo.thumbnail}`}
                alt="Photo thumbnail"
              />
              <p class="photo-date">
                {new Date(photo.createdAt).toLocaleDateString()}
              </p>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

function Dashboard() {
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

      <PhotoGallery />
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
