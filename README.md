# Naive row-by-row PEPS contraction for N-Queens

This crate is a deliberately simple, exact baseline for contracting the
N-Queens constraint PEPS one complete row at a time.

The boundary tensor is stored sparsely as

```text
(occupied_columns, attacked_from_left_diagonals, attacked_from_right_diagonals)
    -> exact integer coefficient
```

Each contraction step visits every column for every non-zero input boundary
entry. A locally forbidden queen placement contributes zero. A valid placement
updates the three open virtual-index masks, and equal outgoing boundary entries
are added. The implementation uses `u128` coefficients and performs no
floating-point arithmetic, SVD, approximation, symmetry reduction, or bond
truncation.

## Run

```powershell
cargo test
cargo run --release -- solve 8 --layers
cargo run --release -- bench 12 --min 4 --repeats 3 --csv
```

`solve --layers` prints the sparse support and transition counts after every
contracted row. `bench` verifies all sizes for which an embedded reference
count is available.

See [`REPORT.md`](REPORT.md) for the PEPS equivalence argument, program design,
and measured benchmark results.

