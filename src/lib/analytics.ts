import { houseEdge } from "@house-edge/analytics";

export { houseEdge };

const viewPaths = new Map<string, string>([
  ["Conflict Autopilot", "/conflict-autopilot"],
  ["Projects", "/projects"],
  ["Overview", "/overview"],
  ["Ports", "/ports"],
  ["Processes", "/processes"],
  ["Favorites", "/favorites"],
  ["History", "/history"],
  ["Check Port", "/check-port"],
  ["Settings", "/settings"],
]);
let currentPath = "/ports";
export function trackView(view: string) {
  const path = viewPaths.get(view);
  if (!path || path === currentPath) return;
  currentPath = path;
  houseEdge.page();
}

// Browser previews may be tracked; native desktop sessions are always excluded.
export function startAnalytics() {
  if (
    "__TAURI_INTERNALS__" in window ||
    !["http:", "https:"].includes(location.protocol)
  )
    return;
  const key = import.meta.env.VITE_HOUSE_EDGE_KEY?.trim();
  const endpoint = import.meta.env.VITE_HOUSE_EDGE_ENDPOINT?.trim();
  const enabled =
    import.meta.env.PROD ||
    import.meta.env.VITE_HOUSE_EDGE_TRACK_DEVELOPMENT === "true";
  if (!enabled || !key || !endpoint) return;
  try {
    const url = new URL(endpoint);
    if (
      !["https:", "http:"].includes(url.protocol) ||
      url.username ||
      url.password
    )
      return;
  } catch {
    return;
  }
  houseEdge.init({
    projectKey: import.meta.env.VITE_HOUSE_EDGE_PROJECT || "port-authority",
    key,
    endpoint,
    version: import.meta.env.VITE_APP_VERSION,
    respectDoNotTrack: true,
    getPath: () => currentPath,
  });
}

if (import.meta.hot) import.meta.hot.dispose(() => houseEdge.destroy());
