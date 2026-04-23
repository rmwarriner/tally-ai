import { useState } from "react";

import { useCommitProposal } from "../../hooks/useCommitProposal";
import { ArtifactCard } from "../artifacts/ArtifactCard";
import { HandoffMessage } from "../onboarding/HandoffMessage";
import { SetupCard } from "../onboarding/SetupCard";
import { AIMessage } from "./AIMessage";
import { DateSeparator } from "./DateSeparator";
import { ProactiveMessage } from "./ProactiveMessage";
import { SystemMessage } from "./SystemMessage";
import { TransactionCard } from "./TransactionCard";
import { UserMessage } from "./UserMessage";
import type { ChatMessage } from "./chatTypes";
import styles from "./MessageList.module.css";

interface MessageListProps {
  messages: ChatMessage[];
  onPromptClick?: (prompt: string) => void;
}

interface TransactionMessageProps {
  message: Extract<ChatMessage, { kind: "transaction" }>;
}

function TransactionMessage({ message }: TransactionMessageProps) {
  const { commit, discard } = useCommitProposal();
  const [isCommitting, setIsCommitting] = useState(false);
  const proposal = message.proposal;
  const state = message.state ?? "posted";
  const isProposal = state === "pending" && proposal !== undefined;

  const handleConfirm = async () => {
    if (!proposal) return;
    setIsCommitting(true);
    try {
      await commit(message.id, proposal);
    } finally {
      setIsCommitting(false);
    }
  };

  return (
    <TransactionCard
      state={state}
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
      onConfirm={isProposal ? handleConfirm : undefined}
      onDiscard={isProposal ? () => discard(message.id) : undefined}
      isCommitting={isCommitting}
      commitError={message.commit_error}
    />
  );
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

function renderMessage(message: ChatMessage, onPromptClick?: (prompt: string) => void) {
  switch (message.kind) {
    case "user":
      return <UserMessage text={message.text} />;
    case "ai":
      return <AIMessage text={message.text} />;
    case "proactive":
      return (
        <ProactiveMessage
          id={message.id}
          text={message.text}
          ts={message.ts}
          advisory_code={message.advisory_code}
        />
      );
    case "system":
      return <SystemMessage text={message.text} tone={message.tone} />;
    case "transaction":
      return <TransactionMessage message={message} />;
    case "artifact":
      return (
        <ArtifactCard title={message.title}>
          {message.content ? (
            <pre className={styles.artifactContent}>{message.content}</pre>
          ) : (
            <p className={styles.artifactPlaceholder} aria-label="Artifact card placeholder">
              Artifact {message.artifact_id}
            </p>
          )}
        </ArtifactCard>
      );
    case "setup_card":
      return (
        <SetupCard variant={message.variant} title={message.title} detail={message.detail} />
      );
    case "handoff":
      return (
        <HandoffMessage
          householdName={message.householdName}
          accountCount={message.accountCount}
          envelopeCount={message.envelopeCount}
          starterPrompts={message.starterPrompts}
          onPromptClick={onPromptClick ?? (() => undefined)}
        />
      );
    default:
      return null;
  }
}

export function MessageList({ messages, onPromptClick }: MessageListProps) {
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
            {renderMessage(message, onPromptClick)}
          </div>
        );
      })}
    </div>
  );
}
