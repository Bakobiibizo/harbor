import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { useIdentityStore } from "./stores";
import { MainLayout } from "./components/layout";
import { CreateIdentity, UnlockIdentity } from "./components/onboarding";
import {
  ChatPage,
  WallPage,
  FeedPage,
  NetworkPage,
  SettingsPage,
} from "./pages";

function LoadingScreen() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800">
      <div className="text-center">
        <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-100 dark:bg-blue-900 mb-4 animate-pulse">
          <span className="text-3xl">🔐</span>
        </div>
        <p className="text-gray-600 dark:text-gray-400">Loading...</p>
      </div>
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
    </BrowserRouter>
  );
}
