import { TransactionCardCorrectionPair } from "./TransactionCardCorrectionPair";
import { TransactionCardPending } from "./TransactionCardPending";
import { TransactionCardPosted } from "./TransactionCardPosted";
import type { TransactionCardProps } from "./TransactionCard.types";
import { TransactionCardVoided } from "./TransactionCardVoided";

export function TransactionCard({
  state,
  transaction,
  replacement,
  onSendMessage,
  onConfirm,
  onDiscard,
  isCommitting,
  commitError,
}: TransactionCardProps) {
  switch (state) {
    case "pending":
      return (
        <TransactionCardPending
          transaction={transaction}
          onSendMessage={onSendMessage}
          onConfirm={onConfirm}
          onDiscard={onDiscard}
          isCommitting={isCommitting}
          commitError={commitError}
        />
      );
    case "voided":
      return <TransactionCardVoided transaction={transaction} />;
    case "correction_pair":
      if (replacement === undefined) {
        return <TransactionCardVoided transaction={transaction} />;
      }
      return <TransactionCardCorrectionPair transaction={transaction} replacement={replacement} />;
    case "posted":
    default:
      return <TransactionCardPosted transaction={transaction} />;
  }
}
