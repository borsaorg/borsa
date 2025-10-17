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
