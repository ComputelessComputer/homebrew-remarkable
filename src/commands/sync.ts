import chalk from "chalk";
import ora from "ora";
import { RemarkableClient } from "../core/RemarkableClient";
import { SyncEngine } from "../core/SyncEngine";
import { NodeFileAdapter } from "../adapters/NodeFileAdapter";
import { loadAuth, loadConfig, loadState, saveState, expandPath } from "../config/store";

export async function syncCommand(options: { force?: boolean; dryRun?: boolean }): Promise<void> {
	const auth = await loadAuth();
	if (!auth?.deviceToken) {
		console.log(chalk.red("Not registered. Run `remarkable-cli init` to connect your device."));
		process.exit(1);
	}

	const config = await loadConfig();
	const state = await loadState();
	const syncFolder = expandPath(config.syncFolder);

	if (options.force) {
		state.syncState = {};
		state.lastSyncTimestamp = 0;
	}

	const client = new RemarkableClient(auth.deviceToken, auth.deviceId);
	const fs = new NodeFileAdapter();
	const engine = new SyncEngine(
		client,
		fs,
		syncFolder,
		config,
		state,
		() => saveState(state),
	);

	const spinner = ora("Starting sync...").start();

	// Poll progress while syncing
	const progressInterval = setInterval(() => {
		const progress = engine.getProgressSnapshot();
		if (!progress) return;

		if (progress.phase === "Authenticating") {
			spinner.text = "Authenticating...";
		} else if (progress.phase === "Listing cloud items") {
			const count = progress.cloudItemCount > 0
				? ` (${progress.inspectedItemCount}/${progress.cloudItemCount})`
				: "";
			const item = progress.currentItem ? ` ${chalk.dim(progress.currentItem)}` : "";
			spinner.text = `Listing cloud items${count}${item}`;
		} else if (progress.phase === "Syncing documents") {
			const count = progress.documentCount > 0
				? ` [${progress.processedDocumentCount}/${progress.documentCount}]`
				: "";
			const item = progress.currentItem ? ` "${progress.currentItem}"` : "";
			spinner.text = `Syncing documents${count}${item}`;
		} else {
			spinner.text = progress.phase;
		}
	}, 200);

	try {
		const report = await engine.sync();
		clearInterval(progressInterval);

		if (report.fatalError) {
			spinner.fail(`Sync failed: ${report.fatalError}`);
			process.exit(1);
		}

		const summary = [
			`${report.cloudItemCount} found`,
			`${report.syncedCount} synced`,
			`${report.skippedCount} unchanged`,
		];
		if (report.failedCount > 0) {
			summary.push(chalk.red(`${report.failedCount} failed`));
		}
		const duration = report.durationMs < 1000
			? `${report.durationMs}ms`
			: `${(report.durationMs / 1000).toFixed(1)}s`;

		spinner.succeed(`Done in ${duration} — ${summary.join(", ")}.`);

		if (report.failedCount > 0) {
			console.log();
			for (const item of report.failedItems.slice(0, 5)) {
				console.log(chalk.red(`  Failed: "${item.name}" — ${item.error}`));
			}
			if (report.failedItems.length > 5) {
				console.log(chalk.dim(`  ... and ${report.failedItems.length - 5} more`));
			}
		}

		if (report.syncedCount > 0) {
			console.log(chalk.dim(`\n  Synced to ${syncFolder}`));
		}
	} catch (err) {
		clearInterval(progressInterval);
		spinner.fail("Sync failed");
		const message = err instanceof Error ? err.message : String(err);
		console.error(chalk.red(`  ${message}`));
		process.exit(1);
	}
}
