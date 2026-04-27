import { useCallback, useEffect } from "react";

import { ChatThread } from "./components/chat/ChatThread";
import { InputBar } from "./components/input/InputBar";
import { useChatPersistence } from "./hooks/useChatPersistence";
import { useSendMessage } from "./hooks/useSendMessage";
import { useSlashDispatch } from "./hooks/useSlashDispatch";
import { useOnboardingEngine } from "./hooks/useOnboardingEngine";
import { HealthSidebar } from "./components/sidebar/HealthSidebar";
import { useUIStore } from "./stores/uiStore";
import styles from "./App.module.css";

export default function App() {
  const sidebarState = useUIStore((state) => state.sidebarState);
  const toggleSidebar = useUIStore((state) => state.toggleSidebar);
  const sendMessage = useSendMessage();
  const dispatchSlash = useSlashDispatch();
  const onboarding = useOnboardingEngine();
  useChatPersistence();

  const onSend = useCallback(
    (text: string) => {
      if (onboarding.isActive) {
        void onboarding.handleInput(text);
        return;
      }
      if (text.startsWith("/")) {
        void dispatchSlash(text);
        return;
      }
      sendMessage(text);
    },
    [onboarding, dispatchSlash, sendMessage],
  );

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
    <div id="app-shell" className={styles.shell}>
      <HealthSidebar state={sidebarState} onToggle={toggleSidebar} />
      <main className={styles.main}>
        <ChatThread
          onPromptClick={onSend}
          onSubmitGnuCashPath={onboarding.handleFilePicked}
          onConfirmMapping={onboarding.handleConfirmMapping}
          onAcceptReconcile={onboarding.handleAcceptReconcile}
          onRollbackReconcile={onboarding.handleRollbackReconcile}
        />
        <InputBar onSend={onSend} isStreaming={false} />
      </main>
    </div>
  );
}
