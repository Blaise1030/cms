import { useEffect } from "react";
import { ConnectionGate } from "@/app/ConnectionGate";
import { HomePage } from "@/app/HomePage";
import { Shell } from "@/app/Shell";
import { useDaemon, useDaemonSnapshot } from "@/core/useDaemon";

export function App() {
  const daemon = useDaemon();
  const { status, config, reason } = useDaemonSnapshot();

  useEffect(() => {
    daemon.start();
    return () => daemon.stop();
  }, [daemon]);

  if (status === "unreachable") {
    return (
      <ConnectionGate
        origin={config.origin}
        reason={reason}
        onRetry={() => daemon.retry()}
        onConnect={(link) => daemon.connectTo(link)}
      />
    );
  }

  if (status !== "open") {
    return (
      <Shell status={status}>
        <p style={{ opacity: 0.6 }}>Connecting to daemon…</p>
      </Shell>
    );
  }

  return (
    <Shell status={status}>
      <HomePage />
    </Shell>
  );
}
