import type { ReactNode } from "react";
import { Badge, Toolbar, Typography } from "@lenso/ui";
import type { ConnectionStatus } from "@/core/types";

export function Shell(props: {
  status: ConnectionStatus;
  children: ReactNode;
}) {
  return (
    <div
      data-lenso-backdrop
      style={{
        minHeight: "100%",
        display: "flex",
        flexDirection: "column",
        background: "var(--lenso-background-grouped-primary)",
      }}
    >
      <header
        style={{
          position: "sticky",
          top: 0,
          zIndex: 10,
          padding: "12px 16px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
        }}
      >
        <Typography.Heading level={1} style={{ fontSize: 22, margin: 0 }}>
          orso
        </Typography.Heading>
        <Toolbar aria-label="Status" edge="soft">
          <Toolbar.Group glass>
            <Toolbar.Button aria-label={`Connection ${props.status}`}>
              <Badge variant="soft" color={props.status === "open" ? "success" : "warning"}>
                <Badge.Label>{props.status}</Badge.Label>
              </Badge>
            </Toolbar.Button>
          </Toolbar.Group>
        </Toolbar>
      </header>
      <main
        style={{
          flex: 1,
          width: "100%",
          maxWidth: 560,
          margin: "0 auto",
          padding: "8px 16px 48px",
          boxSizing: "border-box",
        }}
      >
        {props.children}
      </main>
    </div>
  );
}
