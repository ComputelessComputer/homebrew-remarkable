export interface FileAdapter {
	ensureDir(path: string): Promise<void>;
	writeBinary(path: string, data: ArrayBuffer): Promise<void>;
	writeText(path: string, content: string): Promise<void>;
	readText(path: string): Promise<string>;
	exists(path: string): Promise<boolean>;
	listMarkdownFiles(dir: string): Promise<string[]>;
}
