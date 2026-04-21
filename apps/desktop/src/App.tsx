import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { ChatThread } from "./components/chat/ChatThread";
import { HealthSidebar } from "./components/sidebar/HealthSidebar";
import { useUIStore } from "./stores/uiStore";
import styles from "./App.module.css";

export default function App() {
  const [queryClient] = useState(() => new QueryClient());
  const sidebarState = useUIStore((state) => state.sidebarState);
  const toggleSidebar = useUIStore((state) => state.toggleSidebar);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "b") {
        event.preventDefault();
        toggleSidebar();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [toggleSidebar]);

  return (
    <QueryClientProvider client={queryClient}>
      <div id="app-shell" className={styles.shell}>
        <HealthSidebar state={sidebarState} onToggle={toggleSidebar} />
        <main className={styles.main}>
          <ChatThread />
        </main>
      </div>
    </QueryClientProvider>
  );
}
