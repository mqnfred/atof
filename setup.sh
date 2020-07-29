#!/bin/bash

test -z "$TOOLS_DIR"		&& export TOOLS_DIR="tools"; mkdir -p "$TOOLS_DIR"
test -z "$RUSTUP_INSTALLER"	&& export RUSTUP_INSTALLER="$TOOLS_DIR/rustup-init.sh"
test -z "$RUSTUP_HOME"		&& export RUSTUP_HOME="$TOOLS_DIR/rustup"
test -z "$CARGO_HOME"		&& export CARGO_HOME="$TOOLS_DIR/cargo"
test -z "$RUST_TARGETS"		&& export RUST_TARGETS="x86_64-unknown-linux-gnu x86_64-apple-darwin"
test -z "$RUST_VERSION"		&& export RUST_VERSION="1.45.0"
test -z "$ACTIVATE"		&& export ACTIVATE=activate.sh

# pull rustup installer, for more info visit https://rustup.rs
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$RUSTUP_INSTALLER"
chmod +x "$RUSTUP_INSTALLER"

# install basic rust suite (rustup, rustc, cargo...) and set path accordingly
export RUSTUP_INIT_SKIP_PATH_CHECK=yes
"$RUSTUP_INSTALLER" -y --no-modify-path --default-toolchain none --profile minimal
export PATH="$CARGO_HOME/bin:$PATH"

# install rustc with explicit targets
rustup toolchain install "$RUST_VERSION"
for TARGET in $RUST_TARGETS; do
	rustup target install "$TARGET"
done

# generate activation script
cat <<EOF > "$ACTIVATE"
#!/bin/bash

export RUSTUP_HOME="$RUSTUP_HOME"
export CARGO_HOME="$CARGO_HOME"
export PATH="$CARGO_HOME/bin:\$PATH"
EOF
