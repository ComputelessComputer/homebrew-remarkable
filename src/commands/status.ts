import chalk from "chalk";
import { loadAuth, loadConfig, loadState, expandPath } from "../config/store";

export async function statusCommand(): Promise<void> {
	const auth = await loadAuth();
	const config = await loadConfig();
	const state = await loadState();

	console.log();

	// Connection status
	if (auth?.deviceToken) {
		console.log(`  ${chalk.green("●")} Connected`);
	} else {
		console.log(`  ${chalk.red("●")} Not connected`);
		console.log(`  Run ${chalk.cyan("remarkable-cli init")} to connect.`);
		console.log();
		return;
	}

	// Sync folder
	console.log(`  Sync folder: ${chalk.cyan(expandPath(config.syncFolder))}`);

	// Last sync
	if (state.lastSyncTimestamp > 0) {
		const date = new Date(state.lastSyncTimestamp);
		const relative = getRelativeTime(date);
		console.log(`  Last sync:   ${date.toLocaleString()} (${relative})`);
	} else {
		console.log(`  Last sync:   ${chalk.dim("never")}`);
	}

	// Item counts
	const docs = Object.values(state.syncState).filter(r => r.type === "DocumentType").length;
	const collections = Object.values(state.syncState).filter(r => r.type === "CollectionType").length;
	if (docs > 0 || collections > 0) {
		console.log(`  Documents:   ${docs}`);
		console.log(`  Folders:     ${collections}`);
	}

	// Last report
	if (state.lastSyncReport) {
		const r = state.lastSyncReport;
		const duration = r.durationMs < 1000 ? `${r.durationMs}ms` : `${(r.durationMs / 1000).toFixed(1)}s`;
		console.log();
		console.log(chalk.dim(`  Last sync: ${r.syncedCount} synced, ${r.skippedCount} unchanged, ${r.failedCount} failed (${duration})`));
	}

	// Config
	if (config.excludePdfs) {
		console.log(chalk.dim(`  PDFs excluded`));
	}

	console.log();
}

function getRelativeTime(date: Date): string {
	const now = Date.now();
	const diff = now - date.getTime();
	const minutes = Math.floor(diff / 60_000);
	const hours = Math.floor(diff / 3_600_000);
	const days = Math.floor(diff / 86_400_000);

	if (minutes < 1) return "just now";
	if (minutes < 60) return `${minutes}m ago`;
	if (hours < 24) return `${hours}h ago`;
	return `${days}d ago`;
}
