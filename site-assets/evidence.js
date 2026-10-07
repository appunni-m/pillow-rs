// Measurements are server-rendered; JavaScript only filters, sorts and expands.
document.addEventListener('DOMContentLoaded', () => {
  for (const dashboard of document.querySelectorAll('.benchmark-dashboard')) {
    const search = dashboard.querySelector('#evidence-filter');
    const type = dashboard.querySelector('#bench-group');
    const mode = dashboard.querySelector('#bench-mode');
    const machine = dashboard.querySelector('#bench-machine');
    const subject = dashboard.querySelector('#bench-subject');
    const readerSummary = dashboard.querySelector('#bench-reader-summary');
    const ratioPlot = dashboard.querySelector('.bench-ratio-plot');
    const tables = [...dashboard.querySelectorAll('.bench-comparison')];
    const tableRows = tables.map(table => ({
      body: table.tBodies[0],
      rows: [...table.tBodies[0].querySelectorAll('.bench-workload')],
      columns: [...table.querySelectorAll('[data-subject]')],
      sorters: [...table.querySelectorAll('.bench-sort')],
      sort: null,
      ascending: true,
    }));
    const rows = tableRows.flatMap(group => group.rows);
    const columns = tableRows.flatMap(group => group.columns);
    const details = new Map(rows.map(row => [row, row.nextElementSibling]));
    const cells = row => [...row.querySelectorAll('td[data-subject]')];
    const defaultComparison = subject.value;
    const comparisonOptions = [...subject.options].filter(option => option.dataset.target && option.dataset.baseline);
    const comparisonFor = option => option && option.dataset.target && option.dataset.baseline
      ? { target: option.dataset.target, baseline: option.dataset.baseline }
      : null;
    const comparisonsToShow = () => {
      const selectedComparison = comparisonFor(subject.selectedOptions[0]);
      return selectedComparison ? [selectedComparison] : comparisonOptions.map(comparisonFor).filter(Boolean);
    };
    let selectedPair = null;
    const pairedSubjects = () => {
      const selectedComparison = comparisonFor(subject.selectedOptions[0]);
      return selectedComparison ? new Set([selectedComparison.target, selectedComparison.baseline]) : null;
    };
    const selected = cell => !selectedPair || selectedPair.has(cell.dataset.subject);
    const number = (row, key) => {
      const cell = cells(row).find(item => item.dataset.subject === key);
      return cell?.dataset.value ? Number(cell.dataset.value) : null;
    };
    const targetNames = new Map([...dashboard.querySelectorAll('th[data-subject]')].map(header => [
        header.dataset.subject,
        header.querySelector('button')?.childNodes[0]?.textContent.trim() || header.dataset.subject,
      ]));
    const displayName = key => targetNames.get(key) || key;
    const collectWorkloads = () => {
      const observations = new Map();
      for (const row of rows) {
        if (row.hidden) continue;
        for (const comparison of comparisonsToShow()) {
          const targetCell = cells(row).find(cell => cell.dataset.subject === comparison.target && cell.dataset.role === 'target');
          const baselineCell = cells(row).find(cell => cell.dataset.subject === comparison.baseline);
          if (!targetCell || !baselineCell) continue;
          const ratioKey = comparison.baseline === 'pillow-simd' ? 'ratioPillowSimd' : 'ratioPrimary';
          const qualityKey = comparison.baseline === 'pillow-simd' ? 'qualityPillowSimd' : 'qualityPrimary';
          const ratio = Number(targetCell.dataset[ratioKey]);
          if (targetCell.dataset[qualityKey] !== 'checked' || !Number.isFinite(ratio) || ratio <= 0) continue;
          const baselineTime = Number(baselineCell.dataset.value);
          const targetTime = Number(targetCell.dataset.value);
          if (!Number.isFinite(baselineTime) || baselineTime <= 0 || !Number.isFinite(targetTime) || targetTime <= 0) continue;
          const scope = dashboard.dataset.kind === 'pillow' ? row.dataset.kind : '';
          const key = `${row.dataset.machine}|${comparison.target}|${comparison.baseline}|${scope}`;
          if (!observations.has(key)) observations.set(key, {
            machine: row.dataset.machineLabel,
            subject: comparison.target,
            baseline: comparison.baseline,
            scope,
            workloads: [],
          });
          observations.get(key).workloads.push({
            name: `${row.dataset.name}${row.dataset.mode && row.dataset.mode !== 'Not recorded' ? ` · ${row.dataset.mode}` : ''}`,
            workload: row.dataset.workload,
            ratio,
            baselineTime,
            targetTime,
          });
        }
      }
      return [...observations.values()].sort((a, b) =>
        `${a.machine} ${a.subject} ${a.baseline} ${a.scope}`.localeCompare(`${b.machine} ${b.subject} ${b.baseline} ${b.scope}`));
    };
    const median = values => {
      const sorted = [...values].sort((a, b) => a - b);
      const middle = Math.floor(sorted.length / 2);
      return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
    };
    const speedLabel = ratio => ratio === 1 ? 'Same median time' : `${Number(Math.max(ratio, 1 / ratio).toPrecision(3)).toLocaleString()}× ${ratio > 1 ? 'faster' : 'slower'}`;
    const displayDuration = microseconds => {
      const value = n => Number(n.toPrecision(3)).toLocaleString();
      if (microseconds >= 1_000_000) return `${value(microseconds / 1_000_000)} s`;
      if (microseconds >= 1000) return `${value(microseconds / 1000)} ms`;
      if (microseconds >= 1) return `${value(microseconds)} µs`;
      return `${value(microseconds * 1000)} ns`;
    };
    const appendText = (parent, tag, className, text) => {
      const element = document.createElement(tag);
      if (className) element.className = className;
      element.textContent = text;
      parent.append(element);
      return element;
    };
    const renderReaderSummary = () => {
      if (!readerSummary) return;
      const byRunner = new Map();
      for (const group of collectWorkloads()) {
        const key = `${group.machine}|${group.subject}|${group.baseline}`;
        if (!byRunner.has(key)) byRunner.set(key, { machine: group.machine, subject: group.subject, baseline: group.baseline, scopes: [] });
        byRunner.get(key).scopes.push(group);
      }
      const fragment = document.createDocumentFragment();
      const groups = [...byRunner.values()].sort((a, b) =>
        `${a.machine} ${a.subject} ${a.baseline}`.localeCompare(`${b.machine} ${b.subject} ${b.baseline}`));
      for (const group of groups) {
        const card = document.createElement('article');
        card.className = 'bench-reader-group';
        const heading = document.createElement('div');
        heading.className = 'bench-reader-heading';
        appendText(heading, 'h3', '', group.machine);
        appendText(heading, 'p', 'bench-reader-profile', `${displayName(group.subject)} vs ${displayName(group.baseline)}`);
        card.append(heading);
        const scopes = document.createElement('div');
        scopes.className = 'bench-reader-scopes';
        group.scopes.sort((a, b) => a.scope.localeCompare(b.scope));
        for (const scope of group.scopes) {
          const ratios = scope.workloads.map(item => item.ratio);
          if (!ratios.length) continue;
          const total = ratios.length;
          const faster = ratios.filter(value => value > 1).length;
          const slower = ratios.filter(value => value < 1).length;
          const equal = ratios.filter(value => value === 1).length;
          const section = document.createElement('section');
          section.className = 'bench-reader-scope';
          const heading = appendText(section, 'h4', '', scope.scope === 'operations' ? 'Individual operations' : scope.scope === 'pipelines' ? 'Complete pipelines' : 'Measurements');
          appendText(heading, 'small', '', `${total} matched case${total === 1 ? '' : 's'}`);
          const result = document.createElement('div');
          result.className = 'bench-reader-result';
          appendText(result, 'p', 'bench-reader-verdict', `Faster in ${faster} of ${total} matched cases.`);
          const logMean = ratios.reduce((sum, ratio) => sum + Math.log(ratio), 0) / total;
          appendText(result, 'p', 'bench-reader-average', `Average speedup: ${speedLabel(Math.exp(logMean))}`);
          appendText(result, 'p', 'bench-reader-typical', `Typical case: ${speedLabel(median(ratios))}`);
          const bar = document.createElement('div');
          bar.className = 'bench-outcome-bar';
          bar.setAttribute('role', 'img');
          bar.setAttribute('aria-label', `${faster} faster, ${equal} same, ${slower} slower`);
          for (const [name, count] of [['faster', faster], ['tie', equal], ['slower', slower]]) {
            const segment = document.createElement('span');
            segment.className = name;
            segment.style.width = `${count / total * 100}%`;
            bar.append(segment);
          }
          result.append(bar);
          const labels = document.createElement('p');
          labels.className = 'bench-reader-outcome-labels';
          for (const [name, count, label] of [['faster', faster, 'faster'], ['tie', equal, 'tied'], ['slower', slower, 'slower']]) {
            appendText(labels, 'span', name, `${count} ${label}`);
          }
          result.append(labels);
          const findings = [];
          const slowest = scope.workloads.reduce((best, item) => item.ratio < best.ratio ? item : best, scope.workloads[0]);
          const fastest = scope.workloads.reduce((best, item) => item.ratio > best.ratio ? item : best, scope.workloads[0]);
          if (slowest.ratio < 1) findings.push(['Slowest case', slowest]);
          if (fastest.ratio > 1) findings.push(['Fastest case', fastest]);
          if (findings.length) {
            const list = document.createElement('ul');
            list.className = 'bench-reader-findings';
            for (const [label, item] of findings) {
              const entry = document.createElement('li');
              appendText(entry, 'span', '', label);
              appendText(entry, 'strong', '', item.name || item.workload);
              appendText(entry, 'small', '', `${displayName(group.subject)} took ${displayDuration(item.targetTime)} vs ${displayName(group.baseline)} ${displayDuration(item.baselineTime)} · ${speedLabel(item.ratio)}`);
              list.append(entry);
            }
            result.append(list);
          }
          section.append(result);
          scopes.append(section);
        }
        card.append(scopes);
        fragment.append(card);
      }
      if (!groups.length) appendText(fragment, 'p', 'bench-reader-empty', 'No parity-checked cases match these filters.');
      readerSummary.replaceChildren(fragment);
    };
    const renderWorkloadPlot = () => {
      if (!ratioPlot) return;
      const groups = collectWorkloads();
      const ns = 'http://www.w3.org/2000/svg';
      const svg = document.createElementNS(ns, 'svg');
      const left = 450, right = 1080, valueX = 1090, top = 44, rowHeight = 25, groupGap = 33, width = 1220;
      const totalRows = groups.reduce((count, item) => count + item.workloads.length, 0);
      const height = Math.max(150, top + groups.reduce((count, item) => count + groupGap + rowHeight * item.workloads.length, 0) + 24);
      const xFor = ratio => left + (Math.max(-4, Math.min(4, Math.log2(ratio))) + 4) / 8 * (right - left);
      svg.setAttribute('class', 'bench-workload-plot-svg');
      svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
      svg.setAttribute('role', 'img');
      svg.setAttribute('aria-labelledby', 'bench-workload-plot-title bench-workload-plot-desc');
      const add = (tag, attrs, text) => {
        const element = document.createElementNS(ns, tag);
        for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
        if (text !== undefined) element.textContent = text;
        svg.append(element);
        return element;
      };
      add('title', { id: 'bench-workload-plot-title' }, 'Parity-verified speed ratio for each workload');
      add('desc', { id: 'bench-workload-plot-desc' }, 'Each row is one exact-output-verified workload. The ratio is baseline median time divided by target median time. One times means equal median latency; points to the right are faster and points to the left are slower. Rows are sorted slowest first within each runner and comparison.');
      const tickLabels = new Map([[1 / 16, '1/16×'], [1 / 8, '1/8×'], [1 / 4, '1/4×'], [1 / 2, '1/2×'], [1, '1×'], [2, '2×'], [4, '4×'], [8, '8×'], [16, '16×']]);
      for (const tick of tickLabels.keys()) {
        const x = xFor(tick);
        add('line', { class: tick === 1 ? 'bench-ratio-grid bench-ratio-grid-baseline' : 'bench-ratio-grid', x1: x, y1: 28, x2: x, y2: height - 12 });
        add('text', { class: 'bench-axis-label', x, y: 18, 'text-anchor': 'middle' }, tickLabels.get(tick));
      }
      const baselineDisplay = { pillow: 'Pillow', 'pillow-simd': 'Pillow-SIMD · SSE4' };
      let y = top;
      groups.forEach(item => {
        item.workloads.sort((a, b) => a.ratio - b.ratio || a.name.localeCompare(b.name) || a.workload.localeCompare(b.workload));
        const ratios = item.workloads.map(workload => workload.ratio);
        const logMean = Math.exp(ratios.reduce((sum, value) => sum + Math.log(value), 0) / ratios.length);
        const geomeanLabel = logMean === 1 ? '1×' : speedLabel(logMean);
        const faster = ratios.filter(value => value > 1).length;
        const slower = ratios.filter(value => value < 1).length;
        const equal = ratios.filter(value => value === 1).length;
        const scopeLabel = item.scope ? ` · ${item.scope === 'operations' ? 'individual operations' : 'pipelines'}` : '';
        const label = `${item.machine} · ${targetNames.get(item.subject) || item.subject} vs ${baselineDisplay[item.baseline] || targetNames.get(item.baseline) || item.baseline}${scopeLabel}`;
        add('text', { class: 'bench-ratio-group', x: 8, y, textLength: 420, lengthAdjust: 'spacingAndGlyphs' }, label);
        add('text', { class: 'bench-ratio-group-summary', x: 8, y: y + 13, textLength: 420, lengthAdjust: 'spacingAndGlyphs' }, `${ratios.length} workloads · typical ${speedLabel(median(ratios))} · geomean ${geomeanLabel} · ${faster} faster / ${slower} slower / ${equal} equal`);
        y += groupGap;
        for (const workload of item.workloads) {
          const direction = workload.ratio > 1 ? 'faster' : workload.ratio < 1 ? 'slower' : 'tie';
          const name = workload.name || workload.workload;
          const baselineName = baselineDisplay[item.baseline] || targetNames.get(item.baseline) || item.baseline;
          const targetName = targetNames.get(item.subject) || item.subject;
          const detailsText = `${name}; ${baselineName} median ${workload.baselineTime} µs; ${targetName} median ${workload.targetTime} µs; ${speedLabel(workload.ratio)}`;
          const text = add('text', { class: 'bench-ratio-workload', x: 8, y: y + 4, textLength: 420, lengthAdjust: 'spacingAndGlyphs' }, name);
          const title = document.createElementNS(ns, 'title'); title.textContent = detailsText; text.append(title);
          const point = add('circle', { class: `bench-ratio-point ${direction}`, cx: xFor(workload.ratio), cy: y, r: 4 });
          const pointTitle = document.createElementNS(ns, 'title'); pointTitle.textContent = detailsText; point.append(pointTitle);
          add('text', { class: `bench-ratio-value ${direction}`, x: valueX, y: y + 4 }, speedLabel(workload.ratio));
          y += rowHeight;
        }
      });
      if (!totalRows) add('text', { class: 'bench-empty-plot', x: width / 2, y: height / 2, 'text-anchor': 'middle' }, 'No parity-verified workload pairs match these filters.');
      const previousPlot = ratioPlot.querySelector('svg');
      if (previousPlot) previousPlot.replaceWith(svg);
      else ratioPlot.append(svg);
    };
    const update = () => {
      const query = search.value.trim().toLocaleLowerCase();
      selectedPair = pairedSubjects();
      let visible = 0;
      for (const cell of columns) cell.hidden = !selected(cell);
      for (const row of rows) {
        row.hidden = !row.dataset.search.toLocaleLowerCase().includes(query)
          || !!(type.value && row.dataset.group !== type.value)
          || !!(mode.value && row.dataset.mode !== mode.value)
          || !!(machine.value && row.dataset.machine !== machine.value);
        const detail = details.get(row);
        detail.hidden = row.hidden || row.querySelector('.bench-expand').getAttribute('aria-expanded') !== 'true';
        if (!row.hidden) {
          visible += 1;
        }
        const comparable = cells(row).filter(cell => selected(cell) && cell.dataset.ratio);
        const minimum = Math.min(...comparable.map(cell => Number(cell.dataset.value)));
        const headerFor = key => row.closest('table').querySelector(`thead [data-subject="${CSS.escape(key)}"]`);
        const names = comparable.filter(cell => Number(cell.dataset.value) === minimum).map(cell => {
          const header = headerFor(cell.dataset.subject);
          const name = header.querySelector('button').childNodes[0].textContent;
          const route = cell.querySelector('.bench-route');
          return route ? `${name} (${route.textContent.toLowerCase()})` : name;
        });
        row.querySelector('[data-fastest]').textContent = comparable.length > 1 ? names.join(' / ') : 'Not comparable';
        const notes = cells(row).filter(cell => selected(cell) && cell.dataset.role === 'target').map(cell => cell.dataset.note || 'Not measured');
        row.querySelector('[data-quality-summary]').textContent = [...new Set(notes)].join('; ') || 'Comparison unavailable';
        for (const cell of cells(row)) cell.classList.toggle('bench-lowest', comparable.length > 1 && selected(cell) && !!cell.dataset.ratio && Number(cell.dataset.value) === minimum);
      }
      dashboard.querySelector('#bench-count').textContent = `${visible} of ${rows.length} workloads shown`;
      dashboard.querySelector('#bench-empty').hidden = visible !== 0;
      renderReaderSummary();
      renderWorkloadPlot();
    };
    for (const state of tableRows) {
      const { body, rows: tableWorkloads, sorters } = state;
      for (const button of sorters) {
        button.addEventListener('click', () => {
          state.ascending = state.sort === button.dataset.sort ? !state.ascending : true;
          state.sort = button.dataset.sort;
          for (const sorter of sorters) sorter.closest('th').setAttribute('aria-sort', sorter === button ? (state.ascending ? 'ascending' : 'descending') : 'none');
          const ordered = [...tableWorkloads].sort((a, b) => {
            if (state.sort === 'runner') return a.dataset.machineLabel.localeCompare(b.dataset.machineLabel) * (state.ascending ? 1 : -1);
            if (state.sort === 'name') return a.dataset.name.localeCompare(b.dataset.name) * (state.ascending ? 1 : -1);
            if (state.sort === 'type') return a.dataset.group.localeCompare(b.dataset.group) * (state.ascending ? 1 : -1);
            if (state.sort === 'mode') return a.dataset.mode.localeCompare(b.dataset.mode) * (state.ascending ? 1 : -1);
            const av = number(a, state.sort), bv = number(b, state.sort);
            // Missing times are last in both directions, never treated as zero.
            if (av === null || bv === null) return av === bv ? 0 : av === null ? 1 : -1;
            return (av - bv) * (state.ascending ? 1 : -1);
          });
          for (const row of ordered) body.append(row, details.get(row));
        });
      }
      for (const row of tableWorkloads) {
        const button = row.querySelector('.bench-expand');
        button.hidden = false;
        button.addEventListener('click', () => {
          const expanded = button.getAttribute('aria-expanded') !== 'true';
          button.setAttribute('aria-expanded', String(expanded));
          button.textContent = expanded ? 'Hide details' : 'Details';
          update();
        });
      }
    }
    dashboard.querySelector('#bench-reset').addEventListener('click', () => {
      search.value = ''; type.value = ''; mode.value = ''; machine.value = ''; subject.value = defaultComparison;
      for (const state of tableRows) {
        const { body, rows: tableWorkloads, sorters } = state;
        state.sort = null; state.ascending = true;
        for (const sorter of sorters) sorter.closest('th').setAttribute('aria-sort', 'none');
        for (const row of tableWorkloads) {
          body.append(row, details.get(row));
          const button = row.querySelector('.bench-expand');
          button.setAttribute('aria-expanded', 'false'); button.textContent = 'Details';
        }
      }
      update();
    });
    search.addEventListener('input', update);
    type.addEventListener('change', update);
    mode.addEventListener('change', update);
    machine.addEventListener('change', update);
    subject.addEventListener('change', update);
    dashboard.querySelector('.bench-toolbar').hidden = false;
    update();
  }
});
