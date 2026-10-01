// Measurements are server-rendered; JavaScript only filters, sorts and expands.
document.addEventListener('DOMContentLoaded', () => {
  for (const dashboard of document.querySelectorAll('.benchmark-dashboard')) {
    const search = dashboard.querySelector('#evidence-filter');
    const type = dashboard.querySelector('#bench-group');
    const mode = dashboard.querySelector('#bench-mode');
    const subject = dashboard.querySelector('#bench-subject');
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
    const update = () => {
      const query = search.value.trim().toLocaleLowerCase();
      selectedPair = pairedSubjects();
      const counts = Object.fromEntries(scores.map(score => [score.dataset.scoreSubject, { faster: 0, slower: 0, tie: 0, unavailable: 0 }]));
      let visible = 0;
      for (const cell of columns) cell.hidden = !selected(cell);
      for (const row of rows) {
        row.hidden = !row.dataset.search.toLocaleLowerCase().includes(query)
          || !!(type.value && row.dataset.group !== type.value)
          || !!(mode.value && row.dataset.mode !== mode.value);
        const detail = details.get(row);
        detail.hidden = row.hidden || row.querySelector('.bench-expand').getAttribute('aria-expanded') !== 'true';
        if (!row.hidden) {
          visible += 1;
          for (const cell of cells(row)) {
            if (counts[cell.dataset.subject] && cell.dataset.role === 'target') {
              counts[cell.dataset.subject][cell.dataset.direction] += 1;
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
      dashboard.querySelector('#bench-count').textContent = `${visible} of ${rows.length} workloads shown`;
      dashboard.querySelector('#bench-empty').hidden = visible !== 0;
    };
    for (const state of tableRows) {
      const { body, rows: tableWorkloads, sorters } = state;
      for (const button of sorters) {
        button.addEventListener('click', () => {
          state.ascending = state.sort === button.dataset.sort ? !state.ascending : true;
          state.sort = button.dataset.sort;
          for (const sorter of sorters) sorter.closest('th').setAttribute('aria-sort', sorter === button ? (state.ascending ? 'ascending' : 'descending') : 'none');
          const ordered = [...tableWorkloads].sort((a, b) => {
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
      search.value = ''; type.value = ''; mode.value = ''; subject.value = '';
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
    subject.addEventListener('change', update);
    dashboard.querySelector('.bench-toolbar').hidden = false;
    update();
  }
});
