import chalk from "chalk";
import { clearState, clearAuth } from "../config/store";

export async function resetCommand(options: { auth?: boolean }): Promise<void> {
	await clearState();
	console.log(chalk.green("  Sync state cleared. Next sync will re-download all documents."));

	if (options.auth) {
		await clearAuth();
		console.log(chalk.green("  Device disconnected. Run `remarkable-cli init` to reconnect."));
	}

	console.log();
}
