import { createInterface } from "node:readline/promises";
import { stdin, stdout } from "node:process";
import { exec } from "node:child_process";
import { platform } from "node:os";
import chalk from "chalk";
import ora from "ora";
import { RemarkableClient, generateDeviceId } from "../core/RemarkableClient";
import { saveAuth, saveConfig, getConfigDir } from "../config/store";

const REGISTER_URL = "https://my.remarkable.com/device/desktop/connect";

export async function initCommand(): Promise<void> {
	const rl = createInterface({ input: stdin, output: stdout });

	try {
		console.log();
		console.log(chalk.bold("  Let's connect your reMarkable tablet."));
		console.log();

		// Try to open browser
		openBrowser(REGISTER_URL);
		console.log(`  1. Open ${chalk.cyan(REGISTER_URL)}`);
		console.log("  2. Sign in and copy the one-time code");
		console.log();

		const code = await rl.question("  Enter your one-time code: ");
		if (!code.trim()) {
			console.log(chalk.red("\n  No code entered. Aborting."));
			return;
		}

		const spinner = ora("Registering device...").start();
		const deviceId = generateDeviceId();

		try {
			const deviceToken = await RemarkableClient.register(code, deviceId);

			// Verify the token works
			const client = new RemarkableClient(deviceToken, deviceId);
			await client.refreshToken();

			spinner.succeed("Device registered");

			await saveAuth({ deviceToken, deviceId });
		} catch (err) {
			spinner.fail("Registration failed");
			const message = err instanceof Error ? err.message : String(err);
			console.error(chalk.red(`\n  Error: ${message}`));
			console.log("  Make sure the code is correct and hasn't expired.");
			return;
		}

		console.log();
		const syncFolder = await rl.question(`  Where to save synced notes? [${chalk.dim("~/remarkable")}]: `);
		const folder = syncFolder.trim() || "~/remarkable";

		await saveConfig({ syncFolder: folder, excludePdfs: false });

		console.log();
		console.log(chalk.green(`  Config saved to ${getConfigDir()}`));
		console.log();
		console.log(`  Run ${chalk.cyan("remarkable-cli sync")} to start syncing.`);
		console.log();
	} finally {
		rl.close();
	}
}

function openBrowser(url: string): void {
	const os = platform();
	const cmd = os === "darwin" ? "open" : os === "win32" ? "start" : "xdg-open";

	exec(`${cmd} "${url}"`, () => {
		// Silently fail if browser can't be opened
	});
}
