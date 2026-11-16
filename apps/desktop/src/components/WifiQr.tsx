import { useEffect, useState } from "react";

import { useClient } from "../api/context";
import { useT } from "../i18n";
import { useSettings } from "../settings";

/**
 * A Wireless Router item's QR code: phones join the network by scanning it.
 * Shown right away, or after a click (Settings › Security).
 */
export function WifiQr({ itemId }: { itemId: string }) {
  const client = useClient();
  const t = useT();
  const { settings, loaded } = useSettings();
  const [asked, setAsked] = useState(false);
  const [svg, setSvg] = useState<string | null>(null);
  const show = (loaded && settings.show_wifi_qr) || asked;

  useEffect(() => {
    if (!show) return;
    let live = true;
    client.wifiQr(itemId).then(
      (s) => live && setSvg(s),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [client, itemId, show]);

  if (!loaded && !asked) return null;
  if (!show) {
    return (
      <button type="button" className="ghost" onClick={() => setAsked(true)}>
        {t("itemDetail.showWifiQr")}
      </button>
    );
  }
  if (!svg) return null;
  return (
    <figure className="wifi-qr">
      <img src={`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`} alt={t("itemDetail.wifiQr")} width={180} height={180} />
      <figcaption className="hint">{t("itemDetail.wifiQrHint")}</figcaption>
    </figure>
  );
}
