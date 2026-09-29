import type { ProjectIdentity } from "./types";
function project(
  name: string,
  slug: string,
  type: string,
  frameworks: string[],
): ProjectIdentity {
  return {
    name,
    rootPath: `/Users/developer/Code/${slug}`,
    displayPath: `~/Code/${slug}`,
    projectType: type,
    frameworks,
    repository: {
      provider: "GitHub",
      owner: "Rippley777",
      repository: slug,
      remoteUrl: `github.com/Rippley777/${slug}`,
      webUrl: `https://github.com/Rippley777/${slug}`,
    },
    gitBranch: "main",
    gitDirty: false,
    confidence: "HIGH",
    evidence: "Process working directory",
    manifests: [type === "Rust" ? "Cargo.toml" : "package.json"],
  };
}
export const sampleProjects = [
  project("Shipwreck", "shipwreck", "Node.js", ["Vite", "React"]),
  project("Plant Journal", "plant-journal", "Node.js", ["Next.js", "React"]),
  project("Port Authority", "port-authority", "Rust", []),
];
