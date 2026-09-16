#!/usr/bin/env node
// Verify the results-first reader experience in a real browser.
const { createRequire } = require('node:module');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const root = path.resolve(__dirname, '..');
const puppeteer = createRequire(path.join(root, 'pillow-rs-js/package.json'))('puppeteer');

async function main() {
  const site = path.join(root, 'target/site');
  const server = http.createServer(async (request, response) => {
    try {
      let file = path.resolve(site, '.' + decodeURIComponent(new URL(request.url, 'http://localhost').pathname));
      if (!file.startsWith(site + '/') && file !== site) throw new Error('outside site');
      if ((await fs.stat(file)).isDirectory()) file = path.join(file, 'index.html');
      const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json' };
      response.setHeader('Content-Type', types[path.extname(file)] || 'application/octet-stream');
      response.end(await fs.readFile(file));
    } catch {
      response.writeHead(404); response.end('Not found');
    }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await puppeteer.launch({ headless: true, args: ['--no-sandbox'] });
    const urls = process.argv.slice(2);
    if (!urls.length) urls.push(`http://127.0.0.1:${server.address().port}/benchmarks/`);
    const output = path.join(root, 'target/docs-preview');
    await fs.mkdir(output, { recursive: true });
    for (const [index, url] of urls.entries()) {
      const page = await browser.newPage();
      for (const width of [1440, 390]) {
        await page.setViewport({ width, height: 1000 });
        await page.goto(url, { waitUntil: 'networkidle0' });
        const before = await page.$eval('.evidence-controls output', element => element.textContent);
        await page.type('#evidence-filter', 'no-such-workload');
        const after = await page.$eval('.evidence-controls output', element => element.textContent);
        if (!after.startsWith('0 of')) throw new Error(`Filter failed: ${after}`);
        await page.$eval('#evidence-filter', element => {
          element.value = ''; element.dispatchEvent(new Event('input', { bubbles: true }));
        });
        const size = await page.evaluate(() => {
          const table = document.querySelector('table[data-benchmark]');
          return { body: document.body.scrollWidth, table: table.getBoundingClientRect().width };
        });
        if (size.body > width || (width < 640 && size.table > width)) throw new Error(`Results overflow at ${width}px`);
        await page.screenshot({ path: path.join(output, `benchmarks-${index}-${width}.png`) });
        console.log(`${url}: ${width}px, ${before}, filter and layout passed`);
      }
      await page.close();
    }
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
