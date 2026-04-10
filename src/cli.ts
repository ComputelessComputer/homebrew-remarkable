import { Command } from "commander";
import { loadAuth } from "./config/store";
import { initCommand } from "./commands/init";
import { syncCommand } from "./commands/sync";
import { statusCommand } from "./commands/status";
import { resetCommand } from "./commands/reset";

const program = new Command();

program
	.name("remarkable-cli")
	.description("Sync your reMarkable tablet notes to a local folder")
	.version("0.1.0");

program
	.command("init")
	.description("Connect your reMarkable tablet")
	.action(initCommand);

program
	.command("sync")
	.description("Sync notes from reMarkable cloud")
	.option("--force", "Re-download all documents")
	.option("--dry-run", "List what would sync without downloading")
	.action(syncCommand);

program
	.command("status")
	.description("Show connection and sync status")
	.action(statusCommand);

program
	.command("reset")
	.description("Clear sync state (next sync re-downloads everything)")
	.option("--auth", "Also disconnect the device")
	.action(resetCommand);

// Default action: init if not registered, sync if registered
program.action(async () => {
	const auth = await loadAuth();
	if (!auth?.deviceToken) {
		await initCommand();
	} else {
		await syncCommand({});
	}
});

program.parse();
