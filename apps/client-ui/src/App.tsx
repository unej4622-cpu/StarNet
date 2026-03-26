import { useState } from "react";

function App() {
  const [status] = useState<"disconnected" | "connected">("disconnected");

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        height: "100vh",
        fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        backgroundColor: "#0a0a0a",
        color: "#e0e0e0",
      }}
    >
      <h1
        style={{
          fontSize: "2.5rem",
          fontWeight: 700,
          marginBottom: "0.5rem",
          background: "linear-gradient(135deg, #f093fb 0%, #f5576c 100%)",
          WebkitBackgroundClip: "text",
          WebkitTextFillColor: "transparent",
        }}
      >
        🌐 StarNet Client
      </h1>
      <p style={{ color: "#888", marginBottom: "2rem" }}>
        Remote Desktop — Client Side
      </p>
      <div
        style={{
          padding: "1rem 2rem",
          borderRadius: "8px",
          backgroundColor:
            status === "connected" ? "#1a3a1a" : "#1a1a2e",
          border: `1px solid ${
            status === "connected" ? "#2d6a2d" : "#333"
          }`,
        }}
      >
        <span style={{ fontSize: "0.9rem" }}>
          Status:{" "}
          {status === "connected"
            ? "🟢 Connected to host"
            : "⚪ Not connected"}
        </span>
      </div>
    </div>
  );
}

export default App;
