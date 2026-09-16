// Tables remain readable without JavaScript. Filtering never changes the data.
document.addEventListener('DOMContentLoaded', () => {
  for (const controls of document.querySelectorAll('.evidence-controls')) {
    const input = controls.querySelector('input');
    const output = controls.querySelector('output');
    let sibling = controls.nextElementSibling;
    while (sibling && !sibling.querySelector('tbody') && sibling.tagName !== 'TABLE') {
      sibling = sibling.nextElementSibling;
    }
    const rows = sibling ? [...sibling.querySelectorAll('tbody tr')] : [];
    const update = () => {
      const query = input.value.trim().toLocaleLowerCase();
      let visible = 0;
      for (const row of rows) {
        row.hidden = !row.textContent.toLocaleLowerCase().includes(query);
        if (!row.hidden) visible += 1;
      }
      output.textContent = `${visible} of ${rows.length} rows shown`;
    };
    input.addEventListener('input', update);
    update();
  }
});
