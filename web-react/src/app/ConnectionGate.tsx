import { useState, type FormEvent } from "react";
import { Button, Card, TextField, Typography } from "@lenso/ui";
import type { UnreachableReason } from "@/core/daemon";

export function ConnectionGate(props: {
  origin: string;
  reason: UnreachableReason | null;
  connecting?: boolean;
  onRetry: () => void;
  onConnect: (link: string) => boolean;
}) {
  const [link, setLink] = useState("");
  const [invalid, setInvalid] = useState(false);

  function submit(e: FormEvent) {
    e.preventDefault();
    const value = link.trim();
    if (!value) return;
    try {
      const url = new URL(value);
      if (url.protocol !== "http:" && url.protocol !== "https:") {
        setInvalid(true);
        return;
      }
    } catch {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    if (!props.onConnect(value)) setInvalid(true);
  }

  const refused = props.reason === "rejected";

  return (
    <div
      style={{
        display: "flex",
        minHeight: "100vh",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
      <Card style={{ width: "100%", maxWidth: 420 }} role="alert">
        <Card.Header>
          <Typography.Paragraph size="xs" color="muted">
            Offline
          </Typography.Paragraph>
          <Card.Title>
            {refused ? "Daemon refused the connection" : "Daemon unreachable"}
          </Card.Title>
          <Card.Description>
            {refused ? (
              <>
                The daemon at <Typography.Code>{props.origin}</Typography.Code> is running but
                rejected this app. Open the link printed by <Typography.Code>orso serve</Typography.Code>{" "}
                to refresh the token, or paste it below.
              </>
            ) : (
              <>
                Nothing is answering at <Typography.Code>{props.origin}</Typography.Code>. Start it
                with <Typography.Code>orso serve</Typography.Code>, then retry.
              </>
            )}
          </Card.Description>
        </Card.Header>
        <Card.Content style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <Button variant="prominent" disabled={props.connecting} onClick={props.onRetry}>
            Retry
          </Button>
          <form onSubmit={submit}>
            <TextField fullWidth>
              <TextField.Label>Daemon link</TextField.Label>
              <TextField.Input
                value={link}
                onChange={(e) => setLink((e.target as HTMLInputElement).value)}
                placeholder="http://127.0.0.1:7800/?t=…"
                aria-invalid={invalid || undefined}
              />
              {invalid ? (
                <TextField.Description>Enter a valid http(s) daemon URL</TextField.Description>
              ) : null}
            </TextField>
            <Button type="submit" variant="bordered" style={{ marginTop: 8 }}>
              Connect
            </Button>
          </form>
        </Card.Content>
      </Card>
    </div>
  );
}
