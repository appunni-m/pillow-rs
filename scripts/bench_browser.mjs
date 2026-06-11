#!/usr/bin/env node
/**
 * Browser WASM Benchmark Driver (Puppeteer)
 *
 * Starts a local HTTP server with COOP/COEP headers, launches headless Chrome,
 * and runs TWO benchmark passes:
 *   - "Browser CPU": --disable-webgpu --disable-gpu
 *   - "Browser GPU": default args (WebGPU enabled)
 *
 * Results saved to:
 *   target/benchmarks/browser_cpu.json
 *   target/benchmarks/browser_gpu.json
 *
 * Usage: node scripts/bench_browser.mjs
 */

import { createServer } from 'http';
import { readFileSync, existsSync, mkdirSync, writeFileSync } from 'fs';
import { extname, join, resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const BENCH_PAGE = join(ROOT, 'pillow-rs-js', 'bench_page', 'index.html');
const OUTPUT_DIR = resolve(ROOT, 'target', 'benchmarks');

const PORT = 9342;

const MIME_TYPES = {
    '.html': 'text/html',
    '.js': 'text/javascript',
    '.mjs': 'text/javascript',
    '.wasm': 'application/wasm',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
    '.png': 'image/png',
    '.json': 'application/json',
    '.css': 'text/css',
};

// ── HTTP Server ───────────────────────────────────────────────────

function startServer(port) {
    return new Promise((resolveServer) => {
        const server = createServer((req, res) => {
            // Strip query string
            let filePath = join(ROOT, req.url.split('?')[0]);
            if (req.url === '/') {
                filePath = BENCH_PAGE;
            }

            // Handle favicon silently
            if (req.url === '/favicon.ico') {
                res.writeHead(204); res.end();
                return;
            }

            if (existsSync(filePath)) {
                const ext = extname(filePath);
                const contentType = MIME_TYPES[ext] || 'application/octet-stream';
                res.writeHead(200, {
                    'Content-Type': contentType,
                    'Cross-Origin-Opener-Policy': 'same-origin',
                    'Cross-Origin-Embedder-Policy': 'require-corp',
                    'Access-Control-Allow-Origin': '*',
                });
                res.end(readFileSync(filePath));
            } else {
                res.writeHead(404, { 'Content-Type': 'text/plain' });
                res.end(`Not found: ${req.url}`);
            }
        });

        server.listen(port, () => {
            console.log(`[server] HTTP server listening on http://localhost:${port}`);
            resolveServer(server);
        });
    });
}

// ── Puppeteer Driver ──────────────────────────────────────────────

async function runBrowserPass(puppeteer, label, browserArgs, serverUrl) {
    console.log(`\n[pass] Starting "${label}" pass...`);

    const browser = await puppeteer.launch({
        headless: true,
        args: browserArgs,
    });

    const page = await browser.newPage();
    page.on('console', (msg) => {
        if (msg.type() === 'error') {
            console.error(`  [console] ${msg.text()}`);
        }
    });
    page.on('pageerror', (err) => {
        console.error(`  [pageerror] ${err.message}`);
    });

    console.log(`[pass] Navigating to ${serverUrl} ...`);
    await page.goto(serverUrl, { waitUntil: 'networkidle0', timeout: 60000 });

    console.log('[pass] Waiting for benchmarks to complete...');

    // Wait for "Done" status — poll up to 120 seconds
    const maxWaitMs = 120000;
    const pollIntervalMs = 500;
    let elapsed = 0;
    let statusText = '';

    while (elapsed < maxWaitMs) {
        try {
            statusText = await page.$eval('#status', (el) => el.textContent);
            if (statusText === 'Done' || statusText === 'Failed') {
                break;
            }
        } catch (e) {
            // Page might not be fully loaded yet
        }
        await new Promise((r) => setTimeout(r, pollIntervalMs));
        elapsed += pollIntervalMs;
    }

    if (statusText === 'Failed') {
        const errorText = await page.$eval('#error', (el) => el.textContent);
        console.error(`[pass] "${label}" failed: ${errorText}`);
        await browser.close();
        return null;
    }

    if (statusText !== 'Done') {
        console.warn(`[pass] "${label}" timed out after ${maxWaitMs}ms`);
        await browser.close();
        return null;
    }

    // Extract results JSON
    const resultsJson = await page.$eval('#results', (el) => el.textContent);
    let results;
    try {
        results = JSON.parse(resultsJson);
    } catch (e) {
        console.error(`[pass] Failed to parse results JSON: ${e.message}`);
        await browser.close();
        return null;
    }

    const resultCount = Object.keys(results).length;
    console.log(`[pass] "${label}" complete: ${resultCount} benchmarks recorded`);

    await browser.close();
    return results;
}

// ── Main ──────────────────────────────────────────────────────────

async function main() {
    let puppeteer;
    try {
        puppeteer = await import('puppeteer');
        puppeteer = puppeteer.default;
    } catch (e) {
        console.error('[fatal] puppeteer is not installed.');
        console.error('  Run: npm install --prefix pillow-rs-js puppeteer');
        process.exit(1);
    }

    // Ensure output directory exists
    if (!existsSync(OUTPUT_DIR)) {
        mkdirSync(OUTPUT_DIR, { recursive: true });
    }

    // Start HTTP server
    console.log('[init] Starting HTTP server...');
    const server = await startServer(PORT);
    const serverUrl = `http://localhost:${PORT}/`;

    try {
        // ── Pass 1: Browser CPU ──
        const cpuResults = await runBrowserPass(
            puppeteer,
            'Browser CPU',
            ['--disable-webgpu', '--disable-gpu', '--no-sandbox', '--disable-setuid-sandbox'],
            serverUrl,
        );

        if (cpuResults) {
            const cpuPath = join(OUTPUT_DIR, 'browser_cpu.json');
            writeFileSync(cpuPath, JSON.stringify(cpuResults, null, 2));
            console.log(`[output] CPU results saved to ${cpuPath}`);
        } else {
            console.warn('[output] CPU pass produced no results');
        }

        // ── Pass 2: Browser GPU ──
        const gpuResults = await runBrowserPass(
            puppeteer,
            'Browser GPU',
            ['--no-sandbox', '--disable-setuid-sandbox'],
            serverUrl,
        );

        if (gpuResults) {
            const gpuPath = join(OUTPUT_DIR, 'browser_gpu.json');
            writeFileSync(gpuPath, JSON.stringify(gpuResults, null, 2));
            console.log(`[output] GPU results saved to ${gpuPath}`);
        } else {
            console.warn('[output] GPU pass produced no results');
        }

    } finally {
        server.close();
        console.log('\n[server] HTTP server stopped');
    }

    console.log('\n[done] Browser benchmarks complete');
}

main().catch((err) => {
    console.error('[fatal]', err);
    process.exit(1);
});
