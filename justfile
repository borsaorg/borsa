clippy_flags := '-W clippy::all -W clippy::cargo -W clippy::pedantic -W clippy::nursery -A clippy::multiple-crate-versions -D warnings'

lint crate='':
  cargo clippy --workspace --all-targets --all-features --fix --allow-dirty {{ if crate != "" { "-p " + crate } else { "" } }} -- \
    {{ clippy_flags }}

test crate='' path='':
  cargo nextest run --workspace --all-targets --all-features \
    {{ if crate != "" { "-p " + crate } else { "" } }} \
    {{ if path != "" { "--test " + path } else { "" } }}

fmt:
  cargo fmt --all

examples:
  for f in examples/examples/[0-9][0-9]_*.rs; do \
    name=$(basename "$f" .rs); \
    echo "==> Running example: $name (live)"; \
    cargo run -p borsa-examples --example "$name" || exit 1; \
  done

examples-mock:
  for f in examples/examples/[0-9][0-9]_*.rs; do \
    name=$(basename "$f" .rs); \
    echo "==> Running example: $name (mock)"; \
    BORSA_EXAMPLES_USE_MOCK=1 cargo run -p borsa-examples --example "$name" || exit 1; \
  done
