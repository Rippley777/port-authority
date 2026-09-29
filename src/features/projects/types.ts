export interface RepositoryInfo {
  provider: string | null;
  owner: string | null;
  repository: string;
  remoteUrl: string;
  webUrl: string | null;
}
export interface ProjectIdentity {
  name: string;
  rootPath: string;
  displayPath: string;
  projectType: string | null;
  frameworks: string[];
  repository: RepositoryInfo | null;
  gitBranch: string | null;
  gitDirty: boolean | null;
  confidence: "HIGH" | "MEDIUM" | "LOW";
  evidence: string;
  manifests: string[];
}
export interface RecentProject {
  identity: ProjectIdentity;
  lastObserved: number;
  pinned: boolean;
  knownPorts: number[];
}
export type ProjectAction =
  "editor" | "terminal" | "repository" | "reveal" | "copy";
export const editorNames: Record<string, string> = {
  auto: "Editor",
  vscode: "VS Code",
  insiders: "VS Code Insiders",
  cursor: "Cursor",
  zed: "Zed",
  idea: "IntelliJ IDEA",
  webstorm: "WebStorm",
  pycharm: "PyCharm",
  sublime: "Sublime Text",
  custom: "Custom Editor",
};
export const terminalNames: Record<string, string> = {
  auto: "System default",
  terminal: "Terminal",
  iterm: "iTerm2",
  warp: "Warp",
  ghostty: "Ghostty",
  system: "System terminal",
  "windows-terminal": "Windows Terminal",
  powershell: "PowerShell",
};
export function projectSearch(p: ProjectIdentity) {
  return `${p.name} ${p.rootPath} ${p.displayPath} ${p.repository?.owner ?? ""}/${p.repository?.repository ?? ""} ${p.projectType ?? ""} ${p.frameworks.join(" ")}`.toLowerCase();
}
