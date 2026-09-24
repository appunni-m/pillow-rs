import { createServer } from 'node:http';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { extname, resolve, sep } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer';

const packageRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const timeoutSeconds = Number(process.env.MIGRATION_BROWSER_TIMEOUT ?? 3600);
const gpuProbeTimeoutMilliseconds = Number(
    process.env.MIGRATION_BROWSER_GPU_PROBE_TIMEOUT_MS ?? 10000,
);
let input;

if (!Number.isFinite(timeoutSeconds) || timeoutSeconds <= 0) {
    throw new Error('MIGRATION_BROWSER_TIMEOUT must be a positive number');
}
if (!Number.isFinite(gpuProbeTimeoutMilliseconds) || gpuProbeTimeoutMilliseconds <= 0) {
    throw new Error('MIGRATION_BROWSER_GPU_PROBE_TIMEOUT_MS must be a positive number');
}

function contentType(path) {
    switch (extname(path)) {
        case '.html': return 'text/html; charset=utf-8';
        case '.mjs':
        case '.js': return 'application/javascript; charset=utf-8';
        case '.wasm': return 'application/wasm';
        case '.json': return 'application/json; charset=utf-8';
        default: return 'application/octet-stream';
    }
}

async function readInput() {
    const chunks = [];
    for await (const chunk of process.stdin) {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
    }
    return Buffer.concat(chunks).toString('utf8');
}

function errorEnvelope(error) {
    return {
        error: {
            class: error?.name ?? 'Error',
            message: String(error?.message ?? error),
            stack: String(error?.stack ?? ''),
        },
    };
}

function stringifyParityInput(value) {
    return JSON.stringify(value, (_key, item) => (
        typeof item === 'number' && Object.is(item, -0)
            ? { __pillow_rs_negative_zero__: true }
            : item
    ));
}

function serveStatic(request, response) {
    // Navigating between streamed batches can abort an in-flight module or
    // WASM fetch. Ignore that expected disconnect instead of allowing an
    // asynchronous response error to terminate the worker.
    response.on('error', () => {});
    const requestUrl = new URL(request.url ?? '/', 'http://127.0.0.1');
    if (requestUrl.pathname === '/__pillow_rs_parity_input__') {
        response.writeHead(200, {
            'Content-Type': 'application/json; charset=utf-8',
            'Cache-Control': 'no-store',
        });
        response.end(stringifyParityInput(input));
        return;
    }

    const relativePath = decodeURIComponent(requestUrl.pathname.replace(/^\/+/, ''));
    const path = resolve(packageRoot, relativePath);
    if (path !== packageRoot && !path.startsWith(`${packageRoot}${sep}`)) {
        response.writeHead(403);
        response.end('forbidden');
        return;
    }
    try {
        const body = readFileSync(path);
        if (response.destroyed) return;
        response.writeHead(200, {
            'Content-Type': contentType(path),
            'Cache-Control': 'no-store',
        });
        response.end(body);
    } catch (error) {
        if (response.headersSent || response.destroyed) {
            response.destroy();
            return;
        }
        const status = error?.code === 'ENOENT' ? 404 : 500;
        response.writeHead(status);
        response.end(status === 404 ? 'not found' : String(error?.message ?? error));
    }
}

function listen(server) {
    return new Promise((resolveListen, reject) => {
        server.once('error', reject);
        server.listen(0, '127.0.0.1', () => {
            server.removeListener('error', reject);
            resolveListen(server.address().port);
        });
    });
}

function browserExecutablePath() {
    const explicit = process.env.PUPPETEER_EXECUTABLE_PATH ?? process.env.CHROME_PATH;
    if (explicit && existsSync(explicit)) return explicit;

    // Development machines often have Playwright's Chromium available while
    // Puppeteer's own cache is empty.  Discover that installed browser without
    // making the parity lane depend on a host-specific absolute path.
    const browserRoot = process.env.PLAYWRIGHT_BROWSERS_PATH &&
        process.env.PLAYWRIGHT_BROWSERS_PATH !== '0'
        ? process.env.PLAYWRIGHT_BROWSERS_PATH
        : resolve(homedir(), 'Library', 'Caches', 'ms-playwright');
    const platformDir = process.platform === 'darwin'
        ? process.arch === 'arm64' ? 'chrome-headless-shell-mac-arm64' : 'chrome-headless-shell-mac-x64'
        : process.platform === 'win32'
            ? 'chrome-headless-shell-win64'
            : 'chrome-headless-shell-linux64';
    if (existsSync(browserRoot)) {
        for (const entry of readdirSync(browserRoot).sort().reverse()) {
            const candidate = resolve(browserRoot, entry, platformDir, 'chrome-headless-shell');
            if (existsSync(candidate)) return candidate;
        }
    }

    // Puppeteer's pinned browser downloads live under a different cache
    // layout than Playwright's.  Look there explicitly so a clean hosted
    // runner does not depend on Puppeteer's internal executable-path lookup.
    const puppeteerRoot = process.env.PUPPETEER_CACHE_DIR
        ? resolve(process.env.PUPPETEER_CACHE_DIR)
        : resolve(homedir(), '.cache', 'puppeteer');
    const puppeteerBrowserDir = resolve(puppeteerRoot, 'chrome-headless-shell');
    if (existsSync(puppeteerBrowserDir)) {
        const cachePlatform = process.platform === 'darwin'
            ? process.arch === 'arm64' ? 'mac_arm' : 'mac_x64'
            : process.platform === 'win32' ? 'win64' : 'linux';
        for (const entry of readdirSync(puppeteerBrowserDir).sort().reverse()) {
            const candidate = resolve(
                puppeteerBrowserDir,
                entry,
                cachePlatform,
                platformDir,
                'chrome-headless-shell',
            );
            if (existsSync(candidate)) {
                return candidate;
            }
        }
    }

    const installedCandidates = process.platform === 'darwin'
        ? [
            '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
            '/Applications/Chromium.app/Contents/MacOS/Chromium',
        ]
        : process.platform === 'win32'
            ? []
            : ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'];
    return installedCandidates.find((candidate) => existsSync(candidate)) ?? null;
}

async function runPayload(browser, port, payload, reusablePage = null) {
    // The server endpoint is batch-scoped. Updating this reference before the
    // document is loaded lets worker mode keep one Chromium process while each
    // batch still receives an independent WASM page instance after navigation.
    input = payload;
    const page = reusablePage ?? await browser.newPage();
    const pageErrors = [];
    const onPageError = (error) => pageErrors.push(String(error?.stack ?? error));
    try {
        page.setDefaultNavigationTimeout(timeoutSeconds * 1000);
        page.on('pageerror', onPageError);
        const pageUrl = new URL(
            `http://127.0.0.1:${port}/scripts/browser_parity.html`,
        );
        pageUrl.searchParams.set(
            'gpu_probe_timeout_ms',
            String(gpuProbeTimeoutMilliseconds),
        );
        await page.goto(pageUrl, {
            waitUntil: 'load',
            timeout: timeoutSeconds * 1000,
        });
        await page.waitForFunction(
            () => window.__PILLOW_RS_BROWSER_PARITY__ !== undefined,
            { timeout: timeoutSeconds * 1000 },
        );
        const result = await page.evaluate(() => window.__PILLOW_RS_BROWSER_PARITY__);
        if (result?.error) {
            throw new Error(
                `browser parity adapter failed: ${result.error.class}: ${result.error.message}`,
            );
        }
        if (!result || !result.identity || !Array.isArray(result.results)) {
            const detail = pageErrors.length ? `: ${pageErrors.join(' | ')}` : '';
            throw new Error(`browser parity adapter emitted an invalid envelope${detail}`);
        }
        return result;
    } finally {
        page.off('pageerror', onPageError);
        if (reusablePage === null) await page.close().catch(() => {});
    }
}

async function launchBrowser() {
    const executablePath = browserExecutablePath();
    const launchOptions = {
        headless: true,
        args: [
            '--no-sandbox',
            '--disable-setuid-sandbox',
            // GitHub-hosted Linux runners expose a small /dev/shm.  The
            // parity envelope is intentionally large, so keep Chromium
            // from crashing while materializing the public result.
            '--disable-dev-shm-usage',
            '--enable-unsafe-webgpu',
            '--enable-features=Vulkan',
        ],
    };
    if (executablePath) launchOptions.executablePath = executablePath;
    return puppeteer.launch(launchOptions);
}

async function main() {
    const server = createServer(serveStatic);
    let browser;
    try {
        const port = await listen(server);
        browser = await launchBrowser();
        if (process.env.MIGRATION_BROWSER_WORKER === '1') {
            // Worker mode is used by the streamed parity lane. It keeps the
            // expensive Chromium process alive, but fully reloads one page for
            // every request so process-global RNG semantics remain isolated.
            // Reusing the page also avoids a headless-shell lifecycle bug on
            // some hosted macOS images where closing many short-lived pages
            // can terminate the browser process.
            const page = await browser.newPage();
            // Create the readline consumer only after the reusable page is
            // ready.  Creating it first can consume a caller's already
            // buffered first line before the async iterator is attached.
            const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
            try {
                for await (const line of lines) {
                    if (!line.trim()) continue;
                    let payload;
                    try {
                        payload = JSON.parse(line);
                    } catch (error) {
                        process.stdout.write(`${JSON.stringify(errorEnvelope(error))}\n`);
                        continue;
                    }
                    try {
                        const result = await runPayload(browser, port, payload, page);
                        process.stdout.write(`${JSON.stringify(result)}\n`);
                    } catch (error) {
                        process.stdout.write(`${JSON.stringify(errorEnvelope(error))}\n`);
                    }
                }
            } finally {
                await page.close().catch(() => {});
            }
        } else {
            input = JSON.parse(await readInput());
            const result = await runPayload(browser, port, input);
            process.stdout.write(JSON.stringify(result));
        }
    } finally {
        if (browser) await browser.close();
        await new Promise((resolveClose) => server.close(() => resolveClose()));
    }
}

await main();
