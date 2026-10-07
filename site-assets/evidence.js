// Measurements are server-rendered; JavaScript only filters, sorts and expands.
document.addEventListener('DOMContentLoaded', () => {
  for (const dashboard of document.querySelectorAll('.benchmark-dashboard')) {
    const search = dashboard.querySelector('#evidence-filter');
    const type = dashboard.querySelector('#bench-group');
    const mode = dashboard.querySelector('#bench-mode');
    const machine = dashboard.querySelector('#bench-machine');
    const subject = dashboard.querySelector('#bench-subject');
    const boxplot = dashboard.querySelector('.bench-boxplot');
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
    const renderBoxplot = () => {
      if (!boxplot) return;
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
            const scope = dashboard.dataset.kind === 'pillow'
              ? row.dataset.kind === 'operations' ? 'individual operations' : 'pipeline workflows'
              : '';
            const key = `${row.dataset.machine}|${cell.dataset.subject}|${baselineName}|${scope}`;
            push(key, {
              machine: row.dataset.machineLabel,
              subject: cell.dataset.subject,
              baseline: baselineName,
              scope,
              ratios: [],
            });
            observations.get(key).ratios.push(value);
          }
        }
      }
      const groups = [...observations.values()].map(item => {
        const values = item.ratios.slice().sort((a, b) => a - b);
        const quantile = fraction => {
          if (values.length === 1) return values[0];
          const position = (values.length - 1) * fraction;
          const low = Math.floor(position), high = Math.ceil(position);
          return values[low] + (values[high] - values[low]) * (position - low);
        };
        const geomean = Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length);
        return {
          ...item,
          ratios: values,
          q1: quantile(.25), median: quantile(.5), q3: quantile(.75), geomean,
          faster: values.filter(value => value > 1).length,
          slower: values.filter(value => value < 1).length,
        };
      }).sort((a, b) => `${a.machine} ${a.subject} ${a.baseline}`.localeCompare(`${b.machine} ${b.subject} ${b.baseline}`));
      const ns = 'http://www.w3.org/2000/svg';
      const svg = document.createElementNS(ns, 'svg');
      const left = 385, right = 1070, top = 26, rowHeight = 54, width = 1100;
      const height = Math.max(150, top + Math.max(1, groups.length) * rowHeight + 44);
      const xFor = ratio => left + (Math.max(-4, Math.min(4, Math.log2(ratio))) + 4) / 8 * (right - left);
      svg.setAttribute('class', 'bench-boxplot-svg');
      svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
      svg.setAttribute('role', 'img');
      svg.setAttribute('aria-labelledby', 'bench-boxplot-title bench-boxplot-desc');
      const add = (tag, attrs, text) => {
        const element = document.createElementNS(ns, tag);
        for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
        if (text !== undefined) element.textContent = text;
        svg.append(element);
        return element;
      };
      add('title', { id: 'bench-boxplot-title' }, 'Per-workload speedup distribution');
      add('desc', { id: 'bench-boxplot-desc' }, 'Only exact-output paired workloads are included. Ratios above one are faster. The box shows the first and third quartiles with the median; whiskers reach the furthest observed point within one and a half interquartile ranges.');
      const tickLabels = new Map([[1 / 16, '1/16×'], [1 / 8, '1/8×'], [1 / 4, '1/4×'], [1 / 2, '1/2×'], [1, '1×'], [2, '2×'], [4, '4×'], [8, '8×'], [16, '16×']]);
      for (const tick of tickLabels.keys()) {
        const x = xFor(tick);
        const baselineTick = tick === 1;
        add('line', { x1: x, y1: 18, x2: x, y2: height - 30, stroke: baselineTick ? '#52616d' : '#d1d7dc', 'stroke-width': baselineTick ? 2 : 1 });
        add('text', { class: 'bench-axis-label', x, y: height - 8, 'text-anchor': 'middle' }, tickLabels.get(tick));
      }
      const display = new Map([...targetNames].map(([key, value]) => [key, value.replace('pillow-rs · ', '')]));
      const baselineDisplay = { pillow: 'Pillow', 'pillow-simd': 'Pillow-SIMD · SSE4' };
      groups.forEach((item, index) => {
        const y = top + index * rowHeight + 12;
        const iqr = item.q3 - item.q1;
        const lowerBound = item.q1 - 1.5 * iqr, upperBound = item.q3 + 1.5 * iqr;
        const inliers = item.ratios.filter(value => value >= lowerBound && value <= upperBound);
        const lower = Math.min(...inliers), upper = Math.max(...inliers);
        const baselineLabel = baselineDisplay[item.baseline] || targetNames.get(item.baseline) || item.baseline;
        const scopeLabel = item.scope ? ` · ${item.scope}` : '';
        const label = `${item.machine} · ${display.get(item.subject) || item.subject} vs ${baselineLabel}${scopeLabel}`;
        const speed = item.geomean === 1 ? 'same median time' : item.geomean > 1 ? `${item.geomean.toPrecision(3)}× faster` : `${(1 / item.geomean).toPrecision(3)}× slower`;
        add('text', { class: 'bench-box-label', x: 8, y: y - 3 }, label);
        add('text', { class: 'bench-box-summary', x: 8, y: y + 13 }, `n=${item.ratios.length} · geomean ${speed} · ${item.faster} faster / ${item.slower} slower`);
        add('line', { class: 'bench-whisker', x1: xFor(lower), y1: y + 2, x2: xFor(upper), y2: y + 2 });
        add('line', { class: 'bench-whisker', x1: xFor(lower), y1: y - 5, x2: xFor(lower), y2: y + 9 });
        add('line', { class: 'bench-whisker', x1: xFor(upper), y1: y - 5, x2: xFor(upper), y2: y + 9 });
        add('rect', { class: 'bench-box', x: xFor(item.q1), y: y - 9, width: Math.max(2, xFor(item.q3) - xFor(item.q1)), height: 22 });
        add('line', { class: 'bench-median', x1: xFor(item.median), y1: y - 10, x2: xFor(item.median), y2: y + 14 });
        for (const value of item.ratios) {
          if (value < lowerBound || value > upperBound) {
            const point = add('circle', { class: 'bench-outlier', cx: xFor(value), cy: y + 2, r: 3 });
            const title = document.createElementNS(ns, 'title'); title.textContent = `${value.toPrecision(5)}×`; point.append(title);
          }
        }
      });
      if (!groups.length) add('text', { class: 'bench-empty-plot', x: width / 2, y: height / 2, 'text-anchor': 'middle' }, 'No parity-verified workload pairs match these filters.');
      boxplot.replaceChildren(svg);
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
      renderBoxplot();
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
