import { type UIEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useChatHistory } from "../../hooks/useChatHistory";
import { useChatStore } from "../../stores/chatStore";
import { MessageList } from "./MessageList";
import { NewMessagePill } from "./NewMessagePill";
import styles from "./ChatThread.module.css";

const NEAR_BOTTOM_THRESHOLD = 80;

function isNearBottom(element: HTMLDivElement): boolean {
  return element.scrollHeight - element.scrollTop - element.clientHeight < NEAR_BOTTOM_THRESHOLD;
}

interface ChatThreadProps {
  onPromptClick?: (prompt: string) => void;
  onSubmitGnuCashPath?: (path: string) => void;
  onConfirmMapping?: () => void;
  onAcceptReconcile?: () => void;
  onRollbackReconcile?: () => void;
}

export function ChatThread({ onPromptClick, onSubmitGnuCashPath, onConfirmMapping, onAcceptReconcile, onRollbackReconcile }: ChatThreadProps = {}) {
  const threadRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const previousNewestMessageIdRef = useRef<string | null>(null);
  const previousScrollHeightRef = useRef<number | null>(null);
  const [atBottom, setAtBottom] = useState(true);
  const [showNewMessagePill, setShowNewMessagePill] = useState(false);

  const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = useChatHistory();
  const localMessages = useChatStore((state) => state.localMessages);

  const messages = useMemo(
    () => [...(data?.pages.flatMap((page) => page) ?? []), ...localMessages],
    [data?.pages, localMessages],
  );

  const newestMessageId = messages[messages.length - 1]?.id ?? null;

  const scrollToBottom = useCallback((behavior: ScrollBehavior) => {
    bottomRef.current?.scrollIntoView({ behavior });
  }, []);

  useEffect(() => {
    if (newestMessageId === null) {
      return;
    }

    if (previousNewestMessageIdRef.current === null) {
      previousNewestMessageIdRef.current = newestMessageId;
      scrollToBottom("auto");
      return;
    }

    if (previousNewestMessageIdRef.current === newestMessageId) {
      return;
    }

    previousNewestMessageIdRef.current = newestMessageId;

    if (atBottom) {
      setShowNewMessagePill(false);
      scrollToBottom("smooth");
      return;
    }

    setShowNewMessagePill(true);
  }, [atBottom, newestMessageId, scrollToBottom]);

  useEffect(() => {
    if (isFetchingNextPage) {
      return;
    }

    const previousScrollHeight = previousScrollHeightRef.current;
    const thread = threadRef.current;
    if (previousScrollHeight === null || thread === null) {
      return;
    }

    const delta = thread.scrollHeight - previousScrollHeight;
    thread.scrollTop += delta;
    previousScrollHeightRef.current = null;
  }, [isFetchingNextPage, data?.pages.length]);

  const onScroll = useCallback((event: UIEvent<HTMLDivElement>) => {
    const thread = event.currentTarget;
    const nearBottom = isNearBottom(thread);
    setAtBottom(nearBottom);

    if (nearBottom) {
      setShowNewMessagePill(false);
    }
  }, []);

  const onLoadEarlier = useCallback(async () => {
    const thread = threadRef.current;
    if (thread !== null) {
      previousScrollHeightRef.current = thread.scrollHeight;
    }
    await fetchNextPage();
  }, [fetchNextPage]);

  const onJumpToBottom = useCallback(() => {
    setAtBottom(true);
    setShowNewMessagePill(false);
    scrollToBottom("smooth");
  }, [scrollToBottom]);

  return (
    <section className={styles.wrapper}>
      <div
        ref={threadRef}
        className={styles.thread}
        role="log"
        aria-label="Chat thread"
        aria-live="polite"
        onScroll={onScroll}
      >
        {hasNextPage ? (
          <div className={styles.loadEarlierRow}>
            <button
              type="button"
              className={styles.loadEarlierButton}
              onClick={onLoadEarlier}
              disabled={isFetchingNextPage}
            >
              {isFetchingNextPage ? "Loading earlier messages..." : "Load earlier messages"}
            </button>
          </div>
        ) : null}

        {messages.length > 0 ? (
          <MessageList
            messages={messages}
            onPromptClick={onPromptClick}
            onSubmitGnuCashPath={onSubmitGnuCashPath}
            onConfirmMapping={onConfirmMapping}
            onAcceptReconcile={onAcceptReconcile}
            onRollbackReconcile={onRollbackReconcile}
          />
        ) : (
          <p className={styles.placeholder}>No messages yet.</p>
        )}
        <div ref={bottomRef} />
      </div>

      {showNewMessagePill ? <NewMessagePill onClick={onJumpToBottom} /> : null}
    </section>
  );
}
