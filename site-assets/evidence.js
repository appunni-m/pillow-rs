// Measurements are server-rendered; JavaScript only filters, sorts and expands.
document.addEventListener('DOMContentLoaded', () => {
  for (const dashboard of document.querySelectorAll('.benchmark-dashboard')) {
    const search = dashboard.querySelector('#evidence-filter');
    const group = dashboard.querySelector('#bench-group');
    const subject = dashboard.querySelector('#bench-subject');
    const table = dashboard.querySelector('.bench-comparison');
    const tbody = table.tBodies[0];
    const rows = [...tbody.querySelectorAll('.bench-workload')];
    const columns = [...table.querySelectorAll('[data-subject]')];
    const scores = [...dashboard.querySelectorAll('[data-score-subject]')];
    const baseline = dashboard.dataset.baseline;
    const details = new Map(rows.map(row => [row, row.nextElementSibling]));
    const sorters = [...table.querySelectorAll('.bench-sort')];
    let sort = null;
    let ascending = true;
    const cells = row => [...row.querySelectorAll('td[data-subject]')];
    const selected = cell => !subject.value || [baseline, subject.value].includes(cell.dataset.subject);
    const number = (row, key) => {
      const cell = cells(row).find(item => item.dataset.subject === key);
      return cell?.dataset.value ? Number(cell.dataset.value) : null;
    };
    const update = () => {
      const query = search.value.trim().toLocaleLowerCase();
      const counts = Object.fromEntries(scores.map(score => [score.dataset.scoreSubject, { faster: 0, slower: 0, tie: 0, unavailable: 0 }]));
      let visible = 0;
      for (const cell of columns) cell.hidden = !selected(cell);
      for (const row of rows) {
        row.hidden = !row.dataset.search.toLocaleLowerCase().includes(query) || !!(group.value && row.dataset.group !== group.value);
        const detail = details.get(row);
        detail.hidden = row.hidden || row.querySelector('.bench-expand').getAttribute('aria-expanded') !== 'true';
        if (!row.hidden) {
          visible += 1;
          for (const cell of cells(row)) {
            if (counts[cell.dataset.subject]) counts[cell.dataset.subject][cell.dataset.direction] += 1;
          }
        }
        const comparable = cells(row).filter(cell => selected(cell) && cell.dataset.ratio);
        const minimum = Math.min(...comparable.map(cell => Number(cell.dataset.value)));
        const names = comparable.filter(cell => Number(cell.dataset.value) === minimum).map(cell => {
          const header = [...table.tHead.querySelectorAll('[data-subject]')].find(item => item.dataset.subject === cell.dataset.subject);
          const name = header.querySelector('button').childNodes[0].textContent;
          const route = cell.querySelector('.bench-route');
          return route ? `${name} (${route.textContent.toLowerCase()})` : name;
        });
        row.querySelector('[data-fastest]').textContent = comparable.length > 1 ? names.join(' / ') : 'Not comparable';
        const notes = cells(row).filter(cell => selected(cell) && cell.dataset.subject !== baseline).map(cell => cell.dataset.note || 'Not measured');
        row.querySelector('[data-quality-summary]').textContent = [...new Set(notes)].join('; ');
        for (const cell of cells(row)) cell.classList.toggle('bench-lowest', comparable.length > 1 && selected(cell) && !!cell.dataset.ratio && Number(cell.dataset.value) === minimum);
      }
      for (const score of scores) {
        score.hidden = !!(subject.value && score.dataset.scoreSubject !== subject.value);
        for (const value of score.querySelectorAll('[data-score]')) value.textContent = counts[score.dataset.scoreSubject][value.dataset.score];
      }
      dashboard.querySelector('#bench-count').textContent = `${visible} of ${rows.length} workloads shown`;
      dashboard.querySelector('#bench-empty').hidden = visible !== 0;
    };
    for (const button of sorters) {
      button.addEventListener('click', () => {
        ascending = sort === button.dataset.sort ? !ascending : true;
        sort = button.dataset.sort;
        for (const sorter of sorters) sorter.closest('th').setAttribute('aria-sort', sorter === button ? (ascending ? 'ascending' : 'descending') : 'none');
        const ordered = [...rows].sort((a, b) => {
          if (sort === 'name') return a.dataset.name.localeCompare(b.dataset.name) * (ascending ? 1 : -1);
          const av = number(a, sort), bv = number(b, sort);
          // Missing times are last in both directions, never treated as zero.
          if (av === null || bv === null) return av === bv ? 0 : av === null ? 1 : -1;
          return (av - bv) * (ascending ? 1 : -1);
        });
        for (const row of ordered) tbody.append(row, details.get(row));
      });
    }
    for (const row of rows) {
      const button = row.querySelector('.bench-expand');
      button.hidden = false;
      button.addEventListener('click', () => {
        const expanded = button.getAttribute('aria-expanded') !== 'true';
        button.setAttribute('aria-expanded', String(expanded));
        button.textContent = expanded ? 'Hide details' : 'Details';
        update();
      });
    }
    dashboard.querySelector('#bench-reset').addEventListener('click', () => {
      search.value = ''; group.value = ''; subject.value = ''; sort = null;
      for (const sorter of sorters) sorter.closest('th').setAttribute('aria-sort', 'none');
      for (const row of rows) {
        tbody.append(row, details.get(row));
        const button = row.querySelector('.bench-expand');
        button.setAttribute('aria-expanded', 'false'); button.textContent = 'Details';
      }
      update();
    });
    search.addEventListener('input', update);
    group.addEventListener('change', update);
    subject.addEventListener('change', update);
    dashboard.querySelector('.bench-toolbar').hidden = false;
    update();
  }
});
