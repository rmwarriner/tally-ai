import { useState } from "react";

import { useCommitProposal } from "../../hooks/useCommitProposal";
import { useChatStore } from "../../stores/chatStore";
import { ArtifactCard } from "../artifacts/ArtifactCard";
import { GnuCashMappingCard } from "../artifacts/GnuCashMappingCard";
import { GnuCashReconcileCard } from "../artifacts/GnuCashReconcileCard";
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
  onSubmitGnuCashPath?: (path: string) => void;
  onConfirmMapping?: () => void;
  onAcceptReconcile?: () => void;
  onRollbackReconcile?: () => void;
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

interface RenderOptions {
  onPromptClick?: (prompt: string) => void;
  onSubmitGnuCashPath?: (path: string) => void;
  onConfirmMapping?: () => void;
  onAcceptReconcile?: () => void;
  onRollbackReconcile?: () => void;
  addSystemMessage: (text: string, tone?: "info" | "error") => void;
}

function renderMessage(message: ChatMessage, opts: RenderOptions) {
  const { onPromptClick, onSubmitGnuCashPath, onConfirmMapping, onAcceptReconcile, onRollbackReconcile, addSystemMessage } = opts;
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
          recovery={message.recovery}
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
        <SetupCard
          variant={message.variant}
          title={message.title}
          detail={message.detail}
          onSubmitGnuCashPath={onSubmitGnuCashPath}
        />
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
    case "gnucash_mapping":
      return (
        <GnuCashMappingCard
          plan={message.plan}
          onConfirm={onConfirmMapping ?? (() => undefined)}
          onRequestEdit={() => {
            addSystemMessage(
              "Type 'make <account> a <type>' or 'rename <account> to <new name>' to edit the mapping.",
              "info",
            );
          }}
        />
      );
    case "gnucash_reconcile":
      return (
        <GnuCashReconcileCard
          report={message.report}
          onAccept={onAcceptReconcile ?? (() => undefined)}
          onRollback={onRollbackReconcile ?? (() => undefined)}
        />
      );
    default:
      return null;
  }
}

export function MessageList({ messages, onPromptClick, onSubmitGnuCashPath, onConfirmMapping, onAcceptReconcile, onRollbackReconcile }: MessageListProps) {
  const addSystemMessage = useChatStore((s) => s.addSystemMessage);
  const sorted = [...messages].sort((a, b) => a.ts - b.ts || a.id.localeCompare(b.id));
  const now = new Date();

  const opts: RenderOptions = {
    onPromptClick,
    onSubmitGnuCashPath,
    onConfirmMapping,
    onAcceptReconcile,
    onRollbackReconcile,
    addSystemMessage,
  };

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
            {renderMessage(message, opts)}
          </div>
        );
      })}
    </div>
  );
}
