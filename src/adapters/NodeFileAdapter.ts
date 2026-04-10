import { mkdir, writeFile, readFile, readdir, access } from "node:fs/promises";
import { join } from "node:path";
import type { FileAdapter } from "./FileAdapter";

export class NodeFileAdapter implements FileAdapter {
	async ensureDir(path: string): Promise<void> {
		await mkdir(path, { recursive: true });
	}

	async writeBinary(path: string, data: ArrayBuffer): Promise<void> {
		await this.ensureDir(join(path, ".."));
		await writeFile(path, Buffer.from(data));
	}

	async writeText(path: string, content: string): Promise<void> {
		await this.ensureDir(join(path, ".."));
		await writeFile(path, content, "utf-8");
	}

	async readText(path: string): Promise<string> {
		return readFile(path, "utf-8");
	}

	async exists(path: string): Promise<boolean> {
		try {
			await access(path);
			return true;
		} catch {
			return false;
		}
	}

	async listMarkdownFiles(dir: string): Promise<string[]> {
		try {
			const entries = await readdir(dir, { recursive: true, withFileTypes: true });
			return entries
				.filter(e => e.isFile() && e.name.endsWith(".md"))
				.map(e => {
					const parent = e.parentPath ?? e.path;
					return join(parent, e.name);
				});
		} catch {
			return [];
		}
	}
}
