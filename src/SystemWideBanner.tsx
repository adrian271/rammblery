import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Trust = "checking" | "needed" | "granted";

// System-wide checking control + Accessibility-permission banner.
// M1: enabling flips the Rust poll loop on; it logs focused text to the
// terminal. No in-app UI for the observed text yet (that's M2).
export function SystemWideBanner() {
  const [trust, setTrust] = useState<Trust>("checking");
  const [enabled, setEnabled] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshTrust = useCallback(async (): Promise<boolean> => {
    try {
      const ok = await invoke<boolean>("ax_is_trusted");
      setTrust(ok ? "granted" : "needed");
      return ok;
    } catch {
      // Non-macOS builds don't register these commands.
      setTrust("needed");
      return false;
    }
  }, []);

  useEffect(() => {
    refreshTrust();
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [refreshTrust]);

  const requestPermission = async () => {
    await invoke("ax_request_trust");
    // The grant lands asynchronously (user flips a toggle in System Settings),
    // so poll until it flips, then stop.
    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      const ok = await refreshTrust();
      if (ok && pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    }, 1500);
  };

  const toggleEnabled = async () => {
    if (trust !== "granted") {
      await requestPermission();
      return;
    }
    const next = !enabled;
    await invoke("ax_set_enabled", { enabled: next });
    setEnabled(next);
  };

  return (
    <section className="systemwide">
      <div className="systemwide-row">
        <div>
          <strong>System-wide checking</strong>
          <p className="systemwide-sub">
            {trust === "checking" && "Checking Accessibility permission…"}
            {trust === "needed" &&
              "Needs Accessibility permission to read text in other apps."}
            {trust === "granted" &&
              (enabled
                ? "On — focused text is being read (logged to the terminal in this build)."
                : "Permission granted. Turn on to start checking other apps.")}
          </p>
        </div>
        {trust === "needed" ? (
          <button className="accept" onClick={requestPermission}>
            Grant permission
          </button>
        ) : (
          <button
            className={enabled ? "dismiss" : "accept"}
            disabled={trust !== "granted"}
            onClick={toggleEnabled}
          >
            {enabled ? "Turn off" : "Turn on"}
          </button>
        )}
      </div>
    </section>
  );
}
