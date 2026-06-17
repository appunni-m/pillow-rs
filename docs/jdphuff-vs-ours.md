# IJG jdphuff.c vs Our progressive.rs — AC Refine Comparison

## Key Finding: EOBRUN counts BLOCKS, not positions

Our "unified per-position loop" was wrong. The IJG algorithm has two distinct phases:

## IJG Algorithm (jdphuff.c:497-625)

```
decode_mcu_AC_refine:
  EOBRUN = saved.EOBRUN  // persists across calls

  // ═══ Phase 1: Huffman decode (only when EOBRUN == 0) ═══
  if (EOBRUN == 0) {
      for (k = Ss; k <= Se; k++) {
          decode Huffman symbol → r=run, s=size
          
          if (s != 0) {
              // NEW non-zero coefficient: skip `r` zeros, refine non-zeros along the way
              // Then set coeff[k] = ±(1<<Al)
          } else if (r == 15) {
              // ZRL: skip 16 positions (no EOBRUN change)
          } else {
              // EOB: EOBRUN = (1 << r) + extra_bits
              break;  // → Phase 2 handles remaining band
          }
      }
  }

  // ═══ Phase 2: EOBRUN handler ═══
  if (EOBRUN > 0) {
      for (; k <= Se; k++) {
          if (coeff[k] != 0) {
              read 1 refinement bit
              if bit=1 && (coeff & p1)==0: coeff += ±(1<<Al)
          }
          // NOTE: zero coefficients do NOT consume EOBRUN
      }
      EOBRUN--;  // ONE block consumed
  }

  saved.EOBRUN = EOBRUN
```

## Our Original Code (Phase 1/Phase 2)

```rust
// Phase 1: Normal decode (when eobrun == 0)
if ac_refine_eobrun == 0 {
    while k <= se {
        if coeffs[k] != 0 { refine; k++ }
        else {
            sym = decode
            if s_val != 0 {
                // new coef: skip run zeros, refine non-zeros, place coef
            } else if run == 15 {
                // ZRL
            } else {
                // BUG: missing EOB handling!
                // Original code had nothing here → infinite loop
                // Then we added: EOBRUN = 1<<run + extra; break;
            }
        }
    }
}

// Phase 2: EOBRUN handler
while k <= se {
    if coeffs[k] != 0 { refine }
    k += 1;  // BUG: advances k for zeros too
}
ac_refine_eobrun -= 1;  // CORRECT: per-block decrement
```

## Our "Unified Loop" (WRONG)

```rust
while k <= se {
    if coeffs[k] != 0 { refine; k++ }
    else if eobrun > 0 { eobrun -= 1; k++ }  // WRONG: per-position
    else { decode }
}
// No per-block decrement
```

## What Was Actually Wrong

1. **EOB handling**: Original code didn't handle `size==0, run>0, run!=15` → infinite loop. FIXED by adding `EOBRUN = 1<<run + extra; break;`

2. **Phase 2 zero-position iteration**: The `k += 1` for zero positions is CORRECT in IJG — zeros DON'T consume EOBRUN (EOBRUN counts blocks, not positions). Our "unified loop" changed this to per-position EOBRUN decrement, which was WRONG.

3. **The unified per-position loop was the BAD fix** — it introduced a bug while trying to fix another. We should have kept Phase 1/Phase 2.

## The Correct Fix

Restore the original Phase 1/Phase 2 structure and add EOB handling:

```rust
// Phase 1: Huffman decode (only when eobrun == 0)
if ac_refine_eobrun == 0 {
    while k <= se && k < 64 {
        if coeffs[k] != 0 {
            // refine existing
            let bit = br.read_bits(1)?;
            if bit != 0 { coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 }; }
            k += 1;
        } else {
            let sym = ac_table.decode(&mut br)?;
            let run = (sym >> 4) as usize;
            let size = sym & 0x0F;
            if size == 0 && run == 15 {
                // ZRL: advance 16
                let end = (k + 16).min(se + 1).min(64);
                while k < end {
                    if coeffs[k] != 0 {
                        let bit = br.read_bits(1)?;
                        if bit != 0 { coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 }; }
                    }
                    k += 1;
                }
            } else if size == 0 {
                // EOB: set EOBRUN and break → Phase 2
                ac_refine_eobrun = (1u32 << run) as u32;
                if run > 0 { ac_refine_eobrun |= br.read_bits(run as u32)?; }
                break;
            } else {
                // New non-zero: skip run zeros, refine non-zeros, place coef
                let bit = br.read_bits(1)?;
                let val = if bit != 0 { p1 } else { m1 };
                let mut r = run;
                loop {
                    if k > se || k >= 64 { break; }
                    if coeffs[k] != 0 {
                        let bit = br.read_bits(1)?;
                        if bit != 0 { coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 }; }
                    } else {
                        if r == 0 { break; }
                        r -= 1;
                    }
                    k += 1;
                }
                if k <= se && k < 64 { coeffs[k] = val; k += 1; }
            }
        }
    }
}

// Phase 2: EOBRUN handler
if ac_refine_eobrun > 0 {
    while k <= se && k < 64 {
        if coeffs[k] != 0 {
            let bit = br.read_bits(1)?;
            if bit != 0 { coeffs[k] += if coeffs[k] >= 0 { p1 } else { m1 }; }
        }
        k += 1;  // IJG: zeros advance k but don't consume EOBRUN
    }
    ac_refine_eobrun -= 1;  // one BLOCK consumed
}
```
