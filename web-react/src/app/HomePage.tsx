import { useCallback, useEffect, useState } from "react";
import { Badge, List, Spinner, Typography } from "@lenso/ui";
import { useDaemon, useDaemonSnapshot } from "@/core/useDaemon";
import type { Project } from "@/core/types";

function statusColor(state: Project["devserver"]): "success" | "warning" | "danger" | "default" {
  if (state === "ready") return "success";
  if (state === "starting") return "warning";
  if (state === "crashed") return "danger";
  return "default";
}

export function HomePage() {
  const daemon = useDaemon();
  const { status } = useDaemonSnapshot();
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setProjects(await daemon.api.listProjects());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [daemon.api]);

  useEffect(() => {
    if (status !== "open") return;
    void load();
    const offs = [
      daemon.on("devserver.status", (e) => {
        setProjects((prev) =>
          prev.map((p) =>
            p.id === e.projectId ? { ...p, devserver: e.state, port: e.port ?? p.port } : p,
          ),
        );
      }),
      daemon.onReconnected(() => void load()),
      daemon.on("events.lagged", () => void load()),
    ];
    return () => offs.forEach((off) => off());
  }, [daemon, load, status]);

  if (loading && projects.length === 0) {
    return (
      <div style={{ display: "flex", justifyContent: "center", padding: 48 }}>
        <Spinner aria-label="Loading projects" />
      </div>
    );
  }

  if (error) {
    return (
      <Typography.Paragraph color="default" role="alert">
        {error}
      </Typography.Paragraph>
    );
  }

  if (projects.length === 0) {
    return (
      <Typography.Paragraph color="muted">
        No projects yet. Use the Vue app to create one — this spike only lists them.
      </Typography.Paragraph>
    );
  }

  return (
    <section style={{ width: "100%" }}>
      <Typography.Paragraph
        size="sm"
        color="muted"
        style={{ marginBottom: 8, paddingInline: 4, textTransform: "uppercase", letterSpacing: "0.04em" }}
      >
        Recent
      </Typography.Paragraph>
      <List variant="grouped" data-testid="projects-list" style={{ width: "100%" }}>
        {projects.map((p) => (
          <List.Item key={p.id} itemKey={p.id} textValue={p.name} height="tall">
            <List.Content>
              <List.Title>{p.name}</List.Title>
              <List.Description>{p.repo ?? p.id}</List.Description>
            </List.Content>
            <List.Trailing>
              <Badge color={statusColor(p.devserver)} variant="soft" size="sm">
                <Badge.Label>{p.devserver}</Badge.Label>
              </Badge>
            </List.Trailing>
          </List.Item>
        ))}
      </List>
    </section>
  );
}
