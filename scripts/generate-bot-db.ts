#!/usr/bin/env npx ts-node

import { execFileSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { tmpdir } from "node:os";

const executable = process.platform === "win32" ? "cloud139.exe" : "cloud139";
const releaseBinary = path.resolve(path.dirname(process.argv[1]), "..", "target", "release", executable);
const CLOUD139_BIN = process.env.CLOUD139_BIN
  || (fs.existsSync(releaseBinary) ? releaseBinary : executable);
const PAGE_SIZE = 100;

interface Cloud139Item {
  name: string;
  type: "folder" | "file";
  size: number;
  modified: string;
}

interface Cloud139ListOutput {
  path: string;
  page: number;
  page_size: number;
  total: number;
  items: Cloud139Item[];
}

interface Build {
  fileName: string;
  version: string;
  timestamp: string;
}

interface Recent {
  health: number;
  latestVersion: string;
  errorMessage: string;
  builds: Build[];
}

interface BotDatabase {
  [appName: string]: {
    recent: Recent;
  };
}

function listAll(remotePath: string, output: string): Cloud139Item[] {
  const items = new Map<string, Cloud139Item>();
  let total: number | undefined;
  for (let page = 1; ; page++) {
    fs.rmSync(output, { force: true });
    execFileSync(CLOUD139_BIN, [
      "ls", "--page", String(page), "--page-size", String(PAGE_SIZE), "--output", output, "--", remotePath,
    ], { stdio: "pipe", windowsHide: true });
    const listed = JSON.parse(fs.readFileSync(output, "utf-8")) as Cloud139ListOutput;
    if (listed.page !== page || listed.page_size !== PAGE_SIZE
        || !Number.isSafeInteger(listed.total) || listed.total < 0
        || (total !== undefined && listed.total !== total)
        || !Array.isArray(listed.items)
        || listed.items.length !== Math.min(PAGE_SIZE, listed.total - items.size)) {
      throw new Error(`Incomplete listing for ${remotePath} (page ${page})`);
    }
    total = listed.total;
    for (const item of listed.items) {
      if (!item.name || items.has(item.name)) {
        throw new Error(`Missing or repeated entry in ${remotePath} (page ${page})`);
      }
      items.set(item.name, item);
    }
    if (items.size === total) {
      return [...items.values()];
    }
  }
}

function parseBotFilename(filename: string): { name: string; version: string; author: string } | null {
  const suffix = "（bot）.7z";
  if (!filename.endsWith(suffix)) return null;
  const parts = filename.slice(0, -suffix.length).split("_");
  if (parts.length !== 3 || parts.some(part => !part)) return null;
  return { name: parts[0], version: parts[1], author: parts[2] };
}

function convertToTimestamp(modified: string): string {
  const date = new Date(modified);
  return Number.isNaN(date.getTime()) ? modified : date.toUTCString();
}

function main() {
  const remoteDirectory = process.argv[2];
  if (!remoteDirectory) {
    throw new Error("Usage: npx ts-node scripts/generate-bot-db.ts <remote-directory>");
  }

  const scratch = fs.mkdtempSync(path.join(tmpdir(), "cloud139-bot-"));
  try {
    const listing = path.join(scratch, "listing.json");
    const database: BotDatabase = Object.create(null);
    const categories = listAll(remoteDirectory, listing).filter(item => item.type === "folder");
    for (const category of categories) {
      const categoryPath = `${remoteDirectory.replace(/\/$/, "")}/${category.name}`;
      for (const file of listAll(categoryPath, listing)) {
        if (file.type !== "file") continue;
        const parsed = parseBotFilename(file.name);
        if (!parsed) {
          console.warn(`Skipping invalid bot filename: ${file.name}`);
          continue;
        }

        const recent = (database[parsed.name] ??= {
          recent: { health: 3, latestVersion: parsed.version, errorMessage: "", builds: [] },
        }).recent;
        recent.builds.push({
          fileName: file.name,
          version: parsed.version,
          timestamp: convertToTimestamp(file.modified),
        });
        if (parsed.version.localeCompare(recent.latestVersion, undefined, { numeric: true }) > 0) {
          recent.latestVersion = parsed.version;
        }
      }
    }

    const outputPath = path.resolve("bot-database.json");
    fs.writeFileSync(outputPath, JSON.stringify(database, null, 2), "utf-8");
    console.log(`Wrote ${Object.keys(database).length} apps to ${outputPath}`);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
