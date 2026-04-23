import { ArtifactCard } from "../artifacts/ArtifactCard";
import { AIMessage } from "./AIMessage";
import { DateSeparator } from "./DateSeparator";
import { TransactionCard } from "./TransactionCard";
import { UserMessage } from "./UserMessage";
import type { ChatMessage } from "./chatTypes";
import styles from "./MessageList.module.css";

interface MessageListProps {
  messages: ChatMessage[];
}

function toLocalDateKey(ts: number): string {
  const date = new Date(ts);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function formatDateLabel(ts: number, now: Date): string {
  const messageDate = new Date(ts);
  const messageKey = toLocalDateKey(ts);
  const todayKey = toLocalDateKey(now.getTime());

  if (messageKey === todayKey) {
    return "Today";
  }

  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (messageKey === toLocalDateKey(yesterday.getTime())) {
    return "Yesterday";
  }

  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
  }).format(messageDate);
}

function renderMessage(message: ChatMessage) {
  switch (message.kind) {
    case "user":
      return <UserMessage text={message.text} />;
    case "ai":
      return <AIMessage text={message.text} />;
    case "proactive":
      return <AIMessage text={message.text} />;
    case "transaction":
      return (
        <TransactionCard
          state={message.state ?? "posted"}
          transaction={
            message.transaction ?? {
              id: message.transaction_id,
              payee: "Transaction",
              txn_date: message.ts,
              amount_cents: 0,
              account_name: "Account",
              lines: [],
            }
          }
          replacement={message.replacement}
        />
      );
    case "artifact":
      return (
        <ArtifactCard title={message.title}>
          <p className={styles.artifactPlaceholder} aria-label="Artifact card placeholder">
            Artifact {message.artifact_id}
          </p>
        </ArtifactCard>
      );
    default:
      return null;
  }
}

export function MessageList({ messages }: MessageListProps) {
  const sorted = [...messages].sort((a, b) => a.ts - b.ts || a.id.localeCompare(b.id));
  const now = new Date();

  let lastDateKey: string | null = null;

  return (
    <div className={styles.list}>
      {sorted.map((message) => {
        const dateKey = toLocalDateKey(message.ts);
        const showSeparator = dateKey !== lastDateKey;
        lastDateKey = dateKey;

        return (
          <div key={message.id} className={styles.messageBlock}>
            {showSeparator ? <DateSeparator label={formatDateLabel(message.ts, now)} /> : null}
            {renderMessage(message)}
          </div>
        );
      })}
    </div>
  );
}
