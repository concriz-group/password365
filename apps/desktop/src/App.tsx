import { useCallback, useEffect, useState } from "react";

import type { AppStatus, AutoTypeChoice, SshApproval, UpdateInfo } from "./api/types";
import { useClient } from "./api/context";
import { useT } from "./i18n";
import { ErrorMessage } from "./components/ErrorMessage";
import { Main } from "./screens/Main";
import { Onboarding } from "./screens/Onboarding";
import { Unlock } from "./screens/Unlock";

/** Registers a callback for lock/unlock done outside the window (auto-lock, browser); returns an unsubscribe function. */
export type StatusSubscription = (onChange: () => void) => () => void;

/** Registers a callback for a backend event carrying data; returns an unsubscribe function. */
export type Subscription<T> = (onEvent: (payload: T) => void) => () => void;

/** Routes between onboarding, lock screen and the vault based on backend status. */
export function App({
  subscribeStatus,
  subscribeSync,
  subscribeAutoType,
  subscribeOpenItem,
  subscribeSshApprove,
  subscribeUpdate,
}: {
  subscribeStatus?: StatusSubscription;
  subscribeSync?: StatusSubscription;
  subscribeAutoType?: Subscription<AutoTypeChoice>;
  subscribeOpenItem?: Subscription<{ id: string | null }>;
  subscribeSshApprove?: Subscription<SshApproval>;
  subscribeUpdate?: Subscription<UpdateInfo>;
}) {
  const client = useClient();
  const t = useT();
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [error, setError] = useState<unknown>(null);

  const refresh = useCallback(() => {
    client.status().then(setStatus, setError);
  }, [client]);

  useEffect(refresh, [refresh]);
  useEffect(() => subscribeStatus?.(refresh), [subscribeStatus, refresh]);

  if (error) return <ErrorMessage error={error} />;
  if (!status) return <p className="centered">{t("common.loading")}</p>;
  if (!status.initialized) return <Onboarding onDone={refresh} />;
  if (!status.unlocked) return <Unlock onUnlocked={refresh} />;
  return (
    <Main
      onLocked={refresh}
      subscribeSync={subscribeSync}
      subscribeAutoType={subscribeAutoType}
      subscribeOpenItem={subscribeOpenItem}
      subscribeSshApprove={subscribeSshApprove}
      subscribeUpdate={subscribeUpdate}
    />
  );
}
