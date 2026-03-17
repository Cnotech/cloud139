#!/usr/bin/env npx ts-node

import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

const CLOUD139_BIN = process.env.CLOUD139_BIN || 
  path.join(__dirname, "..", "target", "release", "cloud139.exe") ||
  "cloud139";

interface Cloud139Item {
  name: string;
  type: "folder" | "file";
  size: number;
  modified: string;
}

interface Cloud139ListOutput {
  path: string;
  page?: number;
  page_size?: number;
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

function runCloud139Ls(remotePath: string): Cloud139ListOutput {
  const tempFile = path.join(process.env.TEMP || "/tmp", `cloud139_list_${Date.now()}.json`);
  
  try {
    execSync(`${CLOUD139_BIN} ls "${remotePath}" --output "${tempFile}"`, {
      stdio: "pipe",
      windowsHide: true
    });
    
    if (!fs.existsSync(tempFile)) {
      throw new Error(`Output file not created: ${tempFile}`);
    }
    
    const content = fs.readFileSync(tempFile, "utf-8");
    return JSON.parse(content) as Cloud139ListOutput;
  } catch (error: any) {
    console.error(`Error running cloud139 ls "${remotePath}":`, error.message);
    throw error;
  } finally {
    if (fs.existsSync(tempFile)) {
      try {
        fs.unlinkSync(tempFile);
      } catch {}
    }
  }
}

function isValidBotFile(filename: string): boolean {
  return filename.endsWith("（bot）.7z");
}

function hasTwoUnderscores(filename: string): boolean {
  const withoutExt = filename.replace("（bot）.7z", "");
  const underscoreCount = (withoutExt.match(/_/g) || []).length;
  return underscoreCount === 2;
}

function parseBotFilename(filename: string): { name: string; version: string; author: string } | null {
  const withoutExt = filename.replace("（bot）.7z", "");
  const parts = withoutExt.split("_");
  
  if (parts.length !== 3) {
    return null;
  }
  
  return {
    name: parts[0],
    version: parts[1],
    author: parts[2]
  };
}

function convertToTimestamp(modified: string): string {
  try {
    const date = new Date(modified);
    if (!isNaN(date.getTime())) {
      return date.toUTCString();
    }
  } catch {}
  
  return modified;
}

async function main() {
  const args = process.argv.slice(2);
  
  if (args.length === 0) {
    console.error("Usage: npx ts-node scripts/generate-bot-db.ts <remote-directory>");
    console.error("   or: CLOUD139_BIN=/path/to/cloud139 npx ts-node scripts/generate-bot-db.ts <remote-directory>");
    console.error("\nExample:");
    console.error("  npx ts-node scripts/generate-bot-db.ts /Bot");
    console.error("\nThe script will:");
    console.error("  1. List subdirectories in the given remote directory");
    console.error("  2. Scan each subdirectory for files ending with '（bot）.7z'");
    console.error("  3. Validate filenames have exactly 2 underscores");
    console.error("  4. Parse filenames as 'name_version_author'");
    console.error("  5. Generate bot-database.json in the current directory");
    process.exit(1);
  }
  
  const mainDirectory = args[0];
  
  console.log(`Scanning main directory: ${mainDirectory}`);
  
  const mainList = runCloud139Ls(mainDirectory);
  
  const categoryDirs = mainList.items.filter(item => item.type === "folder");
  
  console.log(`Found ${categoryDirs.length} category directories`);
  
  const database: BotDatabase = {};
  
  for (const category of categoryDirs) {
    const categoryPath = `${mainDirectory}/${category.name}`;
    console.log(`\nProcessing category: ${category.name}`);
    
    const filesList = runCloud139Ls(categoryPath);
    
    for (const file of filesList.items) {
      if (file.type !== "file") continue;
      
      const filename = file.name;
      
      if (!isValidBotFile(filename)) {
        console.warn(`  ⚠️  Skipping non-bot file: ${filename}`);
        continue;
      }
      
      if (!hasTwoUnderscores(filename)) {
        console.warn(`  ⚠️  Invalid filename (not exactly 2 underscores): ${filename}`);
        continue;
      }
      
      const parsed = parseBotFilename(filename);
      if (!parsed) {
        console.warn(`  ⚠️  Failed to parse filename: ${filename}`);
        continue;
      }
      
      const timestamp = convertToTimestamp(file.modified);
      
      if (!database[parsed.name]) {
        database[parsed.name] = {
          recent: {
            health: 3,
            latestVersion: parsed.version,
            errorMessage: "",
            builds: []
          }
        };
      }
      
      const existingBuilds = database[parsed.name].recent.builds;
      const newBuild: Build = {
        fileName: filename,
        version: parsed.version,
        timestamp: timestamp
      };
      
      existingBuilds.push(newBuild);
      
      if (parsed.version.localeCompare(database[parsed.name].recent.latestVersion, undefined, { numeric: true }) > 0) {
        database[parsed.name].recent.latestVersion = parsed.version;
      }
      
      console.log(`  ✓ Added: ${parsed.name} v${parsed.version} (${parsed.author})`);
    }
  }
  
  console.log(`\n=== Generated database with ${Object.keys(database).length} apps ===`);
  
  const outputPath = path.join(process.cwd(), "bot-database.json");
  fs.writeFileSync(outputPath, JSON.stringify(database, null, 2), "utf-8");
  console.log(`\nDatabase written to: ${outputPath}`);
}

main().catch(console.error);
