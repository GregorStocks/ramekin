import { createSignal } from "solid-js";
import "./App.css";
import { DefaultApi, Configuration } from "./generated";

const api = new DefaultApi(
  new Configuration({
    basePath: "",
  }),
);

function App() {
  const [message, setMessage] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  const pingServer = async () => {
    setLoading(true);
    try {
      const data = await api.unauthedPing();
      setMessage(data.message);
    } catch (error) {
      console.error("Failed to ping server:", error);
      setMessage("Error: Failed to ping server");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ padding: "2rem" }}>
      <h1>Ramekin</h1>

      <button
        onClick={pingServer}
        disabled={loading()}
        style={{
          padding: "0.5rem 1rem",
          "font-size": "1rem",
          cursor: loading() ? "not-allowed" : "pointer",
        }}
      >
        {loading() ? "Loading..." : "Ping Server"}
      </button>

      {message() && (
        <div
          style={{
            "margin-top": "1rem",
            padding: "1rem",
            border: "1px solid #ccc",
            "border-radius": "4px",
          }}
        >
          Response: {message()}
        </div>
      )}
    </div>
  );
}

export default App;
