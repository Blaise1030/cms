// Daemon sidecar contract:
// - start: `workbench serve --port 0 [--token <hex>]`
// - ready: parse the stdout line `workbench ready: http://127.0.0.1:<port>/?t=<token>`,
//   then poll `GET /api/health` until it returns `{"status":"ok"}`
// - shutdown: SIGTERM (the daemon exits 0 and reaps its PTY children)

fn main() {
    workbench_desktop_lib::run();
}
