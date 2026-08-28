import { createServer } from 'node:http';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { extname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer';

const packageRoot = resolve(fileURLToPath(new URL('..', import.meta.url)));
const timeoutSeconds = Number(process.env.MIGRATION_BROWSER_TIMEOUT ?? 3600);
let input;

if (!Number.isFinite(timeoutSeconds) || timeoutSeconds <= 0) {
    throw new Error('MIGRATION_BROWSER_TIMEOUT must be a positive number');
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

function serveStatic(request, response) {
    const requestUrl = new URL(request.url ?? '/', 'http://127.0.0.1');
    if (requestUrl.pathname === '/__pillow_rs_parity_input__') {
        response.writeHead(200, {
            'Content-Type': 'application/json; charset=utf-8',
            'Cache-Control': 'no-store',
        });
        response.end(JSON.stringify(input));
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
        response.writeHead(200, {
            'Content-Type': contentType(path),
            'Cache-Control': 'no-store',
        });
        response.end(readFileSync(path));
    } catch (error) {
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

async function main() {
    const server = createServer(serveStatic);
    let browser;
    try {
        input = JSON.parse(await readInput());
        const port = await listen(server);
        const executablePath = browserExecutablePath();
        const launchOptions = {
            headless: true,
            args: [
                '--no-sandbox',
                '--disable-setuid-sandbox',
                '--enable-unsafe-webgpu',
                '--enable-features=Vulkan',
            ],
        };
        if (executablePath) launchOptions.executablePath = executablePath;
        browser = await puppeteer.launch(launchOptions);
        const page = await browser.newPage();
        page.setDefaultNavigationTimeout(timeoutSeconds * 1000);
        const pageErrors = [];
        page.on('pageerror', (error) => pageErrors.push(String(error?.stack ?? error)));
        await page.goto(`http://127.0.0.1:${port}/scripts/browser_parity.html`, {
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
        process.stdout.write(JSON.stringify(result));
    } finally {
        if (browser) await browser.close();
        await new Promise((resolveClose) => server.close(() => resolveClose()));
    }
}

await main();
