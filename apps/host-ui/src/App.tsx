import { useState } from "react";

function App() {
  const [status] = useState<"idle" | "ready">("idle");

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
          background: "linear-gradient(135deg, #667eea 0%, #764ba2 100%)",
          WebkitBackgroundClip: "text",
          WebkitTextFillColor: "transparent",
        }}
      >
        ⭐ StarNet Host
      </h1>
      <p style={{ color: "#888", marginBottom: "2rem" }}>
        Remote Desktop — Host Side
      </p>
      <div
        style={{
          padding: "1rem 2rem",
          borderRadius: "8px",
          backgroundColor: status === "ready" ? "#1a3a1a" : "#1a1a2e",
          border: `1px solid ${status === "ready" ? "#2d6a2d" : "#333"}`,
        }}
      >
        <span style={{ fontSize: "0.9rem" }}>
          Status: {status === "ready" ? "🟢 Ready for connection" : "⚪ Idle"}
        </span>
      </div>
    </div>
  );
}

export default App;
