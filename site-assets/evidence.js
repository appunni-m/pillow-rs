// Measurements are server-rendered; JavaScript only filters, sorts and expands.
document.addEventListener('DOMContentLoaded', () => {
  for (const dashboard of document.querySelectorAll('.benchmark-dashboard')) {
    const search = dashboard.querySelector('#evidence-filter');
    const type = dashboard.querySelector('#bench-group');
    const mode = dashboard.querySelector('#bench-mode');
    const machine = dashboard.querySelector('#bench-machine');
    const subject = dashboard.querySelector('#bench-subject');
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
    const scores = [...dashboard.querySelectorAll('[data-score-subject]')];
    const details = new Map(rows.map(row => [row, row.nextElementSibling]));
    const cells = row => [...row.querySelectorAll('td[data-subject]')];
    let selectedPair = null;
    const pairedSubjects = () => {
      if (!subject.value) return null;
      const header = dashboard.querySelector(`th[data-subject="${CSS.escape(subject.value)}"]`);
      return new Set([subject.value, header?.dataset.baselineFor].filter(Boolean));
    };
    const selected = cell => !selectedPair || selectedPair.has(cell.dataset.subject);
    const number = (row, key) => {
      const cell = cells(row).find(item => item.dataset.subject === key);
      return cell?.dataset.value ? Number(cell.dataset.value) : null;
    };
    const renderWorkloadPlot = () => {
      if (!ratioPlot) return;
      const observations = new Map();
      const targetNames = new Map([...dashboard.querySelectorAll('th[data-subject]')].map(header => [
        header.dataset.subject,
        header.querySelector('button')?.childNodes[0]?.textContent.trim() || header.dataset.subject,
      ]));
      const targetSet = new Set(subject.value ? [subject.value] : [...targetNames.keys()].filter(key => key !== dashboard.dataset.baseline));
      const push = (key, item) => {
        if (!observations.has(key)) observations.set(key, item);
      };
      for (const row of rows) {
        if (row.hidden) continue;
        for (const cell of cells(row)) {
          if (!targetSet.has(cell.dataset.subject) || cell.dataset.role !== 'target') continue;
          for (const [baselineKey, baselineName] of [['ratioPrimary', dashboard.dataset.baseline], ['ratioPillowSimd', 'pillow-simd']]) {
            if (baselineName === 'pillow-simd' && dashboard.dataset.baseline === 'pillow-simd') continue;
            const value = Number(cell.dataset[baselineKey]);
            if (!Number.isFinite(value) || value <= 0) continue;
            const baselineTime = number(row, baselineName);
            const targetTime = Number(cell.dataset.value);
            if (!Number.isFinite(baselineTime) || baselineTime <= 0 || !Number.isFinite(targetTime) || targetTime <= 0) continue;
            const scope = dashboard.dataset.kind === 'pillow'
              ? row.dataset.kind
              : '';
            const key = `${row.dataset.machine}|${cell.dataset.subject}|${baselineName}|${scope}`;
            push(key, {
              machine: row.dataset.machineLabel,
              subject: cell.dataset.subject,
              baseline: baselineName,
              scope,
              workloads: [],
            });
            observations.get(key).workloads.push({
              name: `${row.dataset.name}${row.dataset.mode && row.dataset.mode !== 'Not recorded' ? ` · ${row.dataset.mode}` : ''}`,
              workload: row.dataset.workload,
              ratio: value,
              baselineTime,
              targetTime,
            });
          }
        }
      }
      const groups = [...observations.values()].sort((a, b) =>
        `${a.machine} ${a.subject} ${a.baseline} ${a.scope}`.localeCompare(`${b.machine} ${b.subject} ${b.baseline} ${b.scope}`));
      const ns = 'http://www.w3.org/2000/svg';
      const svg = document.createElementNS(ns, 'svg');
      const left = 450, right = 1080, valueX = 1090, top = 44, rowHeight = 25, groupGap = 33, width = 1220;
      const totalRows = groups.reduce((count, item) => count + item.workloads.length, 0);
      const height = Math.max(150, top + groups.reduce((count, item) => count + groupGap + rowHeight * item.workloads.length, 0) + 24);
      const xFor = ratio => left + (Math.max(-4, Math.min(4, Math.log2(ratio))) + 4) / 8 * (right - left);
      const speedLabel = ratio => ratio === 1 ? 'Same median time' : `${Math.max(ratio, 1 / ratio).toPrecision(3)}× ${ratio > 1 ? 'faster' : 'slower'}`;
      const median = values => {
        const middle = Math.floor(values.length / 2);
        return values.length % 2 ? values[middle] : (values[middle - 1] + values[middle]) / 2;
      };
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
      const counts = Object.fromEntries(scores.map(score => [score.dataset.scoreSubject, { faster: 0, slower: 0, tie: 0, unavailable: 0 }]));
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
          for (const cell of cells(row)) {
            if (counts[cell.dataset.subject] && cell.dataset.role === 'target') {
              counts[cell.dataset.subject][cell.dataset.scoreDirection || cell.dataset.direction] += 1;
            }
          }
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
      for (const score of scores) {
        score.hidden = !!(subject.value && score.dataset.scoreSubject !== subject.value);
        for (const value of score.querySelectorAll('[data-score]')) value.textContent = counts[score.dataset.scoreSubject][value.dataset.score];
      }
      let excluded = 0;
      const targetSet = new Set(subject.value ? [subject.value] : [...dashboard.querySelectorAll('th[data-subject]')]
        .map(header => header.dataset.subject).filter(key => key !== dashboard.dataset.baseline));
      for (const row of rows) {
        if (row.hidden) continue;
        for (const cell of cells(row)) {
          if (!targetSet.has(cell.dataset.subject) || cell.dataset.role !== 'target') continue;
          if (Number(cell.dataset.observedRatioPrimary) > 0 && cell.dataset.qualityPrimary !== 'checked') excluded += 1;
          if (Number(cell.dataset.observedRatioPillowSimd) > 0 && cell.dataset.qualityPillowSimd !== 'checked') excluded += 1;
        }
      }
      const excludedCount = dashboard.querySelector('[data-excluded-count]');
      if (excludedCount) excludedCount.textContent = excluded;
      dashboard.querySelector('#bench-count').textContent = `${visible} of ${rows.length} workloads shown`;
      dashboard.querySelector('#bench-empty').hidden = visible !== 0;
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
      search.value = ''; type.value = ''; mode.value = ''; machine.value = ''; subject.value = '';
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
