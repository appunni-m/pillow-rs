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
        const resetFilters = async () => {
          await page.$eval('#bench-reset', button => button.scrollIntoView({ block: 'center' }));
          await page.click('#bench-reset');
        };
        if (await page.$('pre.api-contract')) {
          const contracts = await page.$$eval('pre.api-contract', nodes => nodes.map(node => ({
            text: node.querySelector('code')?.textContent,
            children: node.querySelector('code')?.childElementCount,
            whitespace: getComputedStyle(node.querySelector('code')).whiteSpace,
          })));
          if (contracts.some(item => !item.text || item.children !== 0 || item.whitespace !== 'pre-wrap')) {
            throw new Error('API contracts are not literal, wrapping code blocks');
          }
          if (contracts.some(item => /&(?:#x27|#124|gt);/.test(item.text))) throw new Error('API contract shows encoded characters');
          await page.$eval('pre.api-contract', element => element.scrollIntoView({ block: 'center', inline: 'center' }));
          const bodyWidth = await page.evaluate(() => document.body.scrollWidth);
          if (bodyWidth > width) throw new Error(`API inventory page overflows at ${width}px`);
          await page.screenshot({ path: path.join(output, `api-contracts-${index}-${width}.png`) });
          console.log(`${url}: ${width}px, ${contracts.length} literal wrapping code blocks passed`);
          continue;
        }
        const before = await page.$eval('#bench-count', element => element.textContent);
        const tableKinds = await page.$$eval('.bench-comparison', tables => tables.map(table => table.closest('[data-table-kind]').dataset.tableKind));
        if (tableKinds.join(',') !== 'pipelines,operations') throw new Error(`Expected pipeline and operation tables, got ${tableKinds.join(',')}`);
        await page.type('#evidence-filter', 'no-such-workload');
        const after = await page.$eval('#bench-count', element => element.textContent);
        if (!after.startsWith('0 of')) throw new Error(`Filter failed: ${after}`);
        await page.$eval('#evidence-filter', element => {
          element.value = ''; element.dispatchEvent(new Event('input', { bubbles: true }));
        });
        const group = await page.$eval('#bench-group', element => element.options[1].value);
        await page.select('#bench-group', group);
        const groupsMatch = await page.$$eval('.bench-workload:not([hidden])', (rows, group) => rows.length > 0 && rows.every(row => row.dataset.group === group), group);
        if (!groupsMatch) throw new Error('Workload group filtering failed');
        await resetFilters();
        const mode = await page.$eval('#bench-mode', element => element.options[1].value);
        await page.select('#bench-mode', mode);
        const modesMatch = await page.$$eval('.bench-workload:not([hidden])', (rows, mode) => rows.length > 0 && rows.every(row => row.dataset.mode === mode), mode);
        if (!modesMatch) throw new Error('Mode filtering failed');
        await resetFilters();
        const runnerOptions = await page.$eval('#bench-machine', element => [...element.options].map(option => ({ value: option.value, label: option.textContent })));
        if (runnerOptions.length > 2) {
          const runner = runnerOptions[1];
          await page.select('#bench-machine', runner.value);
          const runnerMatches = await page.$$eval('.bench-workload:not([hidden])', (rows, value) => rows.length > 0 && rows.every(row => row.dataset.machine === value), runner.value);
          if (!runnerMatches) throw new Error('Runner filtering failed');
          const plotLabels = await page.$$eval('.bench-box-label', nodes => nodes.map(node => node.textContent));
          if (plotLabels.some(label => !label.includes(runner.label))) throw new Error('Box plot retained observations from a filtered runner');
          await resetFilters();
        }
        await page.$eval('.bench-table-scroll', element => { element.scrollLeft = 0; element.scrollTop = 0; });
        await page.evaluate(() => window.scrollTo(0, 0));
        const subject = await page.$eval('#bench-subject', element => element.options[1].value);
        await page.select('#bench-subject', subject);
        const columns = await page.$$eval('.bench-section[data-table-kind="pipelines"] .bench-comparison thead [data-subject]:not([hidden])', nodes => nodes.map(node => node.dataset.subject));
        if (columns.length !== 2 || !columns.includes(subject)) throw new Error('Implementation filtering lost the baseline');
        // Numeric sort must operate on source microseconds, not rounded display
        // strings whose units vary between ns, µs and ms.
        await page.click(`.bench-section[data-table-kind="pipelines"] button[data-sort="${subject}"]`);
        const ascending = await page.$$eval('.bench-section[data-table-kind="pipelines"] .bench-workload', (rows, subject) => rows.map(row => [...row.querySelectorAll('[data-subject]')].find(cell => cell.dataset.subject === subject)).filter(cell => cell.dataset.value).map(cell => Number(cell.dataset.value)), subject);
        if (ascending.some((value, i) => i > 0 && value < ascending[i - 1])) throw new Error('Numeric ascending sort failed');
        await page.click(`.bench-section[data-table-kind="pipelines"] button[data-sort="${subject}"]`);
        const descending = await page.$$eval('.bench-section[data-table-kind="pipelines"] .bench-workload', (rows, subject) => rows.map(row => [...row.querySelectorAll('[data-subject]')].find(cell => cell.dataset.subject === subject)).filter(cell => cell.dataset.value).map(cell => Number(cell.dataset.value)), subject);
        if (descending.some((value, i) => i > 0 && value > descending[i - 1])) throw new Error('Numeric descending sort failed');
        for (const tableKind of tableKinds) {
          for (const [key, field] of [['runner', 'machineLabel'], ['type', 'group'], ['mode', 'mode'], ['name', 'name']]) {
            const section = `.bench-section[data-table-kind="${tableKind}"]`;
            const selector = `${section} button[data-sort="${key}"]`;
            await page.click(selector);
            const ascendingLabels = await page.$$eval(`${section} .bench-workload`, (rows, field) => rows.map(row => row.dataset[field]), field);
            if (ascendingLabels.some((value, i) => i > 0 && value.localeCompare(ascendingLabels[i - 1]) < 0)) throw new Error(`${tableKind} ${key} ascending sort failed`);
            await page.click(selector);
            const descendingLabels = await page.$$eval(`${section} .bench-workload`, (rows, field) => rows.map(row => row.dataset[field]), field);
            if (descendingLabels.some((value, i) => i > 0 && value.localeCompare(descendingLabels[i - 1]) > 0)) throw new Error(`${tableKind} ${key} descending sort failed`);
          }
        }
        await resetFilters();
        await page.click('.bench-section[data-table-kind="pipelines"] button[data-sort="name"]');
        const resetAscending = await page.$$eval('.bench-section[data-table-kind="pipelines"] .bench-workload', rows => rows.map(row => row.dataset.name));
        if (resetAscending.some((value, i) => i > 0 && value.localeCompare(resetAscending[i - 1]) < 0)) throw new Error('Reset did not restore ascending sort');
        await page.$eval('.bench-expand', button => button.scrollIntoView({ block: 'center', inline: 'center' }));
        const expandHit = await page.$eval('.bench-expand', button => {
          const rect = button.getBoundingClientRect();
          const top = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
          return button === top || button.contains(top);
        });
        if (!expandHit) throw new Error('Details button is obscured after sorting');
        await page.click('.bench-expand');
        const detailShown = await page.$eval('.bench-workload', row => row.querySelector('.bench-expand').getAttribute('aria-expanded') === 'true' && !row.nextElementSibling.hidden);
        if (!detailShown) throw new Error('Details detached from the sorted workload');
        await resetFilters();
        const wrapperCount = await page.$$eval('.bench-table-scroll', elements => elements.length);
        if (wrapperCount !== 2) throw new Error(`Reset removed comparison containers: ${wrapperCount}; ${await page.title()}`);
        await page.$eval('.bench-table-scroll', element => { element.scrollLeft = 0; element.scrollTop = 0; });
        await page.evaluate(() => window.scrollTo(0, 0));
        const size = await page.evaluate(() => {
          const wrapper = document.querySelector('.bench-table-scroll');
          return { body: document.body.scrollWidth, wrapper: wrapper.getBoundingClientRect().width };
        });
        await page.screenshot({ path: path.join(output, `benchmarks-${index}-${width}.png`) });
        if (size.body > width || size.wrapper > width) throw new Error(`Page overflows at ${width}px: ${JSON.stringify(size)}`);
        // Horizontal overflow is intentional inside the table, never the page.
        const mobileScroll = await page.$eval('.bench-table-scroll', element => {
          element.scrollLeft = element.scrollWidth;
          const name = element.querySelector('tbody th[scope="row"]');
          return (element.scrollWidth <= element.clientWidth || element.scrollLeft > 0)
            && Math.abs(name.getBoundingClientRect().left - element.getBoundingClientRect().left) < 3;
        });
        if (!mobileScroll) throw new Error('Comparison columns cannot be reached');
        console.log(`${url}: ${width}px, ${before}; filtering, sorting, expansion and layout passed`);
      }
      await page.setJavaScriptEnabled(false);
      await page.goto(url, { waitUntil: 'networkidle0' });
      if (!(await page.$('.bench-workload .bench-time, pre.api-contract code'))) throw new Error('Documentation content requires JavaScript');
      await page.close();
    }
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
