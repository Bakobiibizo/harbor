import { type ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { useIdentityStore } from "../../stores";

interface MainLayoutProps {
  children: ReactNode;
}

const navItems = [
  { to: "/chat", label: "Chat", icon: "💬" },
  { to: "/wall", label: "Wall", icon: "📝" },
  { to: "/feed", label: "Feed", icon: "📰" },
  { to: "/network", label: "Network", icon: "🌐" },
];

export function MainLayout({ children }: MainLayoutProps) {
  const { state, lock } = useIdentityStore();

  const identity = state.status === "unlocked" ? state.identity : null;

  return (
    <div className="flex h-screen bg-gray-100 dark:bg-gray-900">
      {/* Sidebar */}
      <aside className="w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 flex flex-col">
        {/* App header */}
        <div className="p-4 border-b border-gray-200 dark:border-gray-700">
          <h1 className="text-xl font-bold text-gray-900 dark:text-white">
            P2P Chat
          </h1>
          {identity && (
            <p className="text-sm text-gray-500 dark:text-gray-400 truncate mt-1">
              {identity.displayName}
            </p>
          )}
        </div>

        {/* Navigation */}
        <nav className="flex-1 p-4 space-y-1">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                `flex items-center px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
                  isActive
                    ? "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200"
                    : "text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700"
                }`
              }
            >
              <span className="mr-3">{item.icon}</span>
              {item.label}
            </NavLink>
          ))}
        </nav>

        {/* User section */}
        <div className="p-4 border-t border-gray-200 dark:border-gray-700">
          <NavLink
            to="/settings"
            className={({ isActive }) =>
              `flex items-center px-3 py-2 rounded-lg text-sm font-medium transition-colors mb-2 ${
                isActive
                  ? "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200"
                  : "text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700"
              }`
            }
          >
            <span className="mr-3">⚙️</span>
            Settings
          </NavLink>

          {identity && (
            <button
              onClick={() => lock()}
              className="w-full flex items-center px-3 py-2 rounded-lg text-sm font-medium text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-700 transition-colors"
            >
              <span className="mr-3">🔒</span>
              Lock
            </button>
          )}
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto">
        {children}
      </main>
    </div>
  );
}
