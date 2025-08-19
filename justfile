default:
    just -l -u

# tags the newest release in the changelog
tag-release:
    cargo test
    cargo fmt --check

    svbump write "$(cargo run -- version latest)" package.version Cargo.toml
    cargo check

    git commit Cargo.toml Cargo.lock CHANGELOG.md -m "chore: Release changelog version $(svbump read package.version Cargo.toml)"
    git tag "v$(svbump read package.version Cargo.toml)"

    @echo "tagged v$(svbump read package.version Cargo.toml)"
    @echo
    @echo "run this to release it:"
    @echo
    @echo "  git push origin HEAD --tags"

# installs the package
install:
    cargo install --path .
