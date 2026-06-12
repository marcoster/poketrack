import { readdir, writeFile, mkdir } from 'fs/promises';
import { join } from 'path';

const SCRIPT_DIR = import.meta.dir;
const PROJECT_ROOT = join(SCRIPT_DIR, '..');
const DATA_DIR = join(PROJECT_ROOT, 'data', 'cards-database', 'data');
const DATA_ASIA_DIR = join(PROJECT_ROOT, 'data', 'cards-database', 'data-asia');
const OUTPUT_BASE = join(PROJECT_ROOT, 'cards-database-json');

async function exportDir(dir: string, outputDir: string) {
    await mkdir(outputDir, { recursive: true });
    const entries = await readdir(dir, { withFileTypes: true });

    for (const entry of entries) {
        const fullPath = join(dir, entry.name);
        if (entry.isDirectory()) {
            await exportDir(fullPath, join(outputDir, entry.name));
        } else if (entry.name.endsWith('.ts') && !entry.name.endsWith('.d.ts')) {
            try {
                const mod = await import(fullPath + '?t=' + Date.now());
                const jsonPath = join(outputDir, entry.name.replace('.ts', '.json'));
                await writeFile(jsonPath, JSON.stringify(mod.default, null, 2));
            } catch (e) {
                console.error(`Error processing ${fullPath}:`, e);
            }
        }
    }
}

async function main() {
    console.log('Exporting English sets (data/)...');
    await exportDir(DATA_DIR, join(OUTPUT_BASE, 'en'));

    console.log('Exporting Asian sets (data-asia/)...');
    await exportDir(DATA_ASIA_DIR, join(OUTPUT_BASE, 'ja'));

    console.log('Export complete! JSON files written to:', OUTPUT_BASE);
}

main();
