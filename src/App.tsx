import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { Toaster } from "react-hot-toast";
import { useIdentityStore } from "./stores";
import { MainLayout } from "./components/layout";
import { CreateIdentity, UnlockIdentity } from "./components/onboarding";
import { HarborIcon } from "./components/icons";
import {
  ChatPage,
  WallPage,
  FeedPage,
  NetworkPage,
  SettingsPage,
} from "./pages";

function LoadingScreen() {
  return (
    <div
      className="min-h-screen flex items-center justify-center"
      style={{
        background: "linear-gradient(135deg, hsl(220 91% 8%) 0%, hsl(262 60% 12%) 50%, hsl(220 91% 8%) 100%)",
      }}
    >
      <div className="text-center">
        {/* Animated logo container */}
        <div className="relative mb-8">
          {/* Outer glow ring */}
          <div
            className="absolute inset-0 rounded-full animate-pulse"
            style={{
              background: "radial-gradient(circle, hsl(var(--harbor-primary) / 0.3) 0%, transparent 70%)",
              transform: "scale(2)",
            }}
          />
          {/* Logo */}
          <div
            className="relative w-20 h-20 rounded-2xl flex items-center justify-center mx-auto"
            style={{
              background: "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))",
              boxShadow: "0 8px 32px hsl(var(--harbor-primary) / 0.4)",
            }}
          >
            <HarborIcon className="w-12 h-12 text-white" />
          </div>
        </div>

        {/* Loading text */}
        <h2
          className="text-xl font-semibold mb-2"
          style={{ color: "hsl(var(--harbor-text-primary))" }}
        >
          Harbor
        </h2>
        <p
          className="text-sm mb-6"
          style={{ color: "hsl(var(--harbor-text-tertiary))" }}
        >
          Initializing secure connection...
        </p>

        {/* Loading bar */}
        <div
          className="w-48 h-1 rounded-full mx-auto overflow-hidden"
          style={{ background: "hsl(var(--harbor-surface-2))" }}
        >
          <div
            className="h-full rounded-full"
            style={{
              background: "linear-gradient(90deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))",
              animation: "loading-bar 1.5s ease-in-out infinite",
            }}
          />
        </div>
      </div>

      {/* CSS animation for loading bar */}
      <style>{`
        @keyframes loading-bar {
          0% { width: 0%; margin-left: 0%; }
          50% { width: 60%; margin-left: 20%; }
          100% { width: 0%; margin-left: 100%; }
        }
      `}</style>
    </div>
  );
}

function AppContent() {
  const { state, initialize } = useIdentityStore();

  useEffect(() => {
    initialize();
  }, [initialize]);

  // Loading state
  if (state.status === "loading") {
    return <LoadingScreen />;
  }

  // No identity - show create screen
  if (state.status === "no_identity") {
    return <CreateIdentity />;
  }

  // Identity locked - show unlock screen
  if (state.status === "locked") {
    return <UnlockIdentity />;
  }

  // Identity unlocked - show main app
  return (
    <MainLayout>
      <Routes>
        <Route path="/chat" element={<ChatPage />} />
        <Route path="/wall" element={<WallPage />} />
        <Route path="/feed" element={<FeedPage />} />
        <Route path="/network" element={<NetworkPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/chat" replace />} />
      </Routes>
    </MainLayout>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <AppContent />
      <Toaster
        position="bottom-right"
        toastOptions={{
          duration: 3000,
          style: {
            background: "hsl(222 41% 13%)",
            color: "hsl(220 14% 96%)",
            border: "1px solid hsl(222 30% 22%)",
            borderRadius: "12px",
            padding: "12px 16px",
            fontSize: "14px",
            boxShadow: "0 10px 40px rgba(0, 0, 0, 0.4)",
          },
          success: {
            iconTheme: {
              primary: "hsl(152 69% 40%)",
              secondary: "white",
            },
          },
          error: {
            iconTheme: {
              primary: "hsl(0 84% 60%)",
              secondary: "white",
            },
          },
        }}
      />
    </BrowserRouter>
  );
}
