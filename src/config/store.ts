import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { homedir } from "node:os";
import type { AuthConfig, UserConfig, SyncState } from "../core/types";

const CONFIG_DIR = join(homedir(), ".config", "remarkable-cli");

export function getConfigDir(): string {
	return CONFIG_DIR;
}

export function expandPath(p: string): string {
	if (p.startsWith("~/")) {
		return join(homedir(), p.slice(2));
	}
	return p;
}

async function ensureConfigDir(): Promise<void> {
	await mkdir(CONFIG_DIR, { recursive: true });
}

async function readJson<T>(filename: string): Promise<T | null> {
	try {
		const content = await readFile(join(CONFIG_DIR, filename), "utf-8");
		return JSON.parse(content) as T;
	} catch {
		return null;
	}
}

async function writeJson(filename: string, data: unknown): Promise<void> {
	await ensureConfigDir();
	await writeFile(join(CONFIG_DIR, filename), JSON.stringify(data, null, 2) + "\n", "utf-8");
}

// ── Auth ────────────────────────────────────────────────────────────────────

export async function loadAuth(): Promise<AuthConfig | null> {
	return readJson<AuthConfig>("auth.json");
}

export async function saveAuth(auth: AuthConfig): Promise<void> {
	await writeJson("auth.json", auth);
}

export async function clearAuth(): Promise<void> {
	await writeJson("auth.json", { deviceToken: "", deviceId: "" });
}

// ── Config ──────────────────────────────────────────────────────────────────

const DEFAULT_CONFIG: UserConfig = {
	syncFolder: "~/remarkable",
	excludePdfs: false,
};

export async function loadConfig(): Promise<UserConfig> {
	const config = await readJson<UserConfig>("config.json");
	return { ...DEFAULT_CONFIG, ...config };
}

export async function saveConfig(config: UserConfig): Promise<void> {
	await writeJson("config.json", config);
}

// ── State ───────────────────────────────────────────────────────────────────

const DEFAULT_STATE: SyncState = {
	lastSyncTimestamp: 0,
	syncState: {},
};

export async function loadState(): Promise<SyncState> {
	const state = await readJson<SyncState>("state.json");
	return { ...DEFAULT_STATE, ...state };
}

export async function saveState(state: SyncState): Promise<void> {
	await writeJson("state.json", state);
}

export async function clearState(): Promise<void> {
	await writeJson("state.json", DEFAULT_STATE);
}
