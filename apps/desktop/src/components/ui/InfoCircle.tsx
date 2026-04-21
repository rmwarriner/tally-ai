import { useId, useState } from "react";

import { Tooltip } from "./Tooltip";
import styles from "./InfoCircle.module.css";

interface InfoCircleProps {
  tooltip?: string;
  onClick?: () => void;
  "aria-label"?: string;
  className?: string;
}

export function InfoCircle({ tooltip, onClick, "aria-label": ariaLabel, className }: InfoCircleProps) {
  const tooltipId = useId();
  const label = ariaLabel ?? "More information";
  const [tooltipVisible, setTooltipVisible] = useState(false);

  const wrapperClassName = `${styles.root} ${className ?? ""}`.trim();
  const describedBy = tooltip ? tooltipId : undefined;

  return (
    <span
      className={wrapperClassName}
      onMouseEnter={() => setTooltipVisible(true)}
      onMouseLeave={() => setTooltipVisible(false)}
      onFocus={() => setTooltipVisible(true)}
      onBlur={() => setTooltipVisible(false)}
    >
      {onClick ? (
        <button
          type="button"
          className={`${styles.circle} ${styles.button}`}
          onClick={onClick}
          aria-label={label}
          aria-describedby={describedBy}
        >
          <span aria-hidden="true">ⓘ</span>
        </button>
      ) : (
        <span
          className={`${styles.circle} ${styles.icon}`}
          role="img"
          aria-label={label}
          aria-describedby={describedBy}
        >
          <span aria-hidden="true">ⓘ</span>
        </span>
      )}
      {tooltip ? <Tooltip id={tooltipId} text={tooltip} visible={tooltipVisible} /> : null}
    </span>
  );
}

export type { InfoCircleProps };
