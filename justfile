# Resets the db
reset-db:
    cargo sqlx database reset --force -y

clippy:
    cargo clippy --all-features --all-targets -- -D warnings

# Runs the same checks as CI — use before pushing
local-ci:
    @echo "==> fmt"
    cargo fmt --check
    @echo "==> audit"
    cargo audit
    @echo "==> deny"
    cargo deny check
    @echo "==> clippy"
    SQLX_OFFLINE=true cargo clippy -- -D warnings
    @echo "==> test"
    SQLX_OFFLINE=true cargo test
    @echo "==> build"
    SQLX_OFFLINE=true cargo build --release
    @echo "😈 All checks passed."

# Installs git hooks via pre-commit (run once after cloning)
setup-hooks:
    pre-commit install --hook-type pre-commit --hook-type pre-push
    @echo "Git hooks installed."

# Runs CI checks then commits and pushes
push msg: local-ci
    git add .
    git commit -m "{{msg}}"
    git push

# Starts all servises
start-all:
    docker compose up -d
    cargo run

# Resets and starts the database
nuke-start:
    docker compose down -v
    docker compose up -d
    sqlx migrate run

# Removes tracking for a file and adds it to gitignore
gitignore path:
    echo "\n{{path}}" >> .gitignore
    git rm --cached "{{path}}"
    git add .gitignore
    git commit -m "Removed cached file {{path}}"
    git push

# Exposes the backend to a public API
ngrok:
    # ngrok http http://localhost:3000
    # ngrok http http://setigerous-tamela-agitable.ngrok-free.dev
    ngrok http --domain=setigerous-tamela-agitable.ngrok-free.dev 3000

# Use this when your computer just started
cold-start:
    @echo "Starting docker deamon.."
    colima start

    @echo "Starting servcies..."
    docker compose up -d

    @echo "Starting backend"
    cargo run
