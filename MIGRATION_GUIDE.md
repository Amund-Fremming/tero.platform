# Migration Guide: Feature-Folder Architecture

## For Developers Working on Existing Branches

If you have an active branch that was created before this refactoring, follow these steps:

### 1. Update Your Branch

```bash
# Fetch the latest changes
git fetch origin

# Rebase your branch (recommended) or merge
git rebase origin/main
# or
git merge origin/main
```

### 2. Fix Import Conflicts

You'll likely see import errors. Use this mapping to update your imports:

#### API Layer → Feature Handlers
```rust
# Old
use crate::api::game::game_routes;
use crate::api::user::{protected_auth_routes, public_auth_routes};
use crate::api::health::health_routes;

# New
use crate::features::game::handlers::game_routes;
use crate::features::user::handlers::{protected_auth_routes, public_auth_routes};
use crate::features::health::handlers::health_routes;
```

#### Models → Common or Feature Models
```rust
# Old (shared models)
use crate::models::error::ServerError;
use crate::models::app_state::AppState;
use crate::models::auth::Claims;

# New
use crate::common::error::ServerError;
use crate::common::app_state::AppState;
use crate::common::auth::Claims;

# Old (feature-specific models)
use crate::models::game_base::{GameBase, GameType};
use crate::models::user::{Permission, SubjectId};

# New
use crate::features::game::models::{GameBase, GameType};
use crate::features::user::models::{Permission, SubjectId};
```

#### Database Layer → Feature Repository
```rust
# Old
use crate::db::game_base::create_game;
use crate::db::user::get_user_by_id;

# New
use crate::features::game::repository::create_game;
use crate::features::user::repository::get_user_by_id;
```

#### Services → Common Services
```rust
# Old
use crate::service::cache::GustCache;
use crate::service::key_vault::KeyVault;

# New
use crate::common::services::cache::GustCache;
use crate::common::services::key_vault::KeyVault;
```

#### Middleware
```rust
# Old
use crate::api::auth_mw::auth_mw;
use crate::api::webhook_mw::webhook_mw;

# New
use crate::common::middleware::auth::auth_mw;
use crate::common::middleware::webhook::webhook_mw;
```

### 3. Regenerate SQLx Cache

After rebasing/merging, regenerate the SQLx offline cache:

```bash
# Make sure DATABASE_URL is set
export DATABASE_URL="postgresql://user:password@localhost/dbname"

# Regenerate cache
cargo sqlx prepare

# Verify it works
cargo check
```

### 4. Quick Reference: Where Things Are Now

| Old Location | New Location | Notes |
|-------------|-------------|-------|
| `src/api/*.rs` | `src/features/*/handlers.rs` | HTTP handlers |
| `src/db/*.rs` | `src/features/*/repository.rs` | Database queries |
| `src/models/*.rs` | `src/features/*/models.rs` OR `src/common/*.rs` | Depends if feature-specific or shared |
| `src/service/*.rs` | `src/common/services/*.rs` | Shared services |
| `src/api/*_mw.rs` | `src/common/middleware/*.rs` | Middleware |

### 5. Adding New Features

Follow the pattern:

```
src/features/your_feature/
├── mod.rs           # Exports handlers, models, repository
├── handlers.rs      # HTTP handlers (routes, request/response)
├── models.rs        # Domain models and DTOs
└── repository.rs    # Database operations
```

Example `mod.rs`:
```rust
pub mod handlers;
pub mod models;
pub mod repository;
```

Then add to `src/features/mod.rs`:
```rust
pub mod your_feature;
```

### 6. Common Patterns

#### In handlers.rs:
```rust
use crate::{
    common::{
        app_state::AppState,
        error::ServerError,
        auth::Claims,
    },
    features::your_feature::{
        models::YourModel,
        repository,
    },
};

// Use repository functions directly (they're imported)
async fn handler(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, ServerError> {
    let result = repository::your_db_function(state.get_pool()).await?;
    Ok(Json(result))
}
```

#### In repository.rs:
```rust
use crate::{
    common::error::ServerError,
    features::your_feature::models::YourModel,
};
use sqlx::{Pool, Postgres};

pub async fn your_db_function(pool: &Pool<Postgres>) -> Result<YourModel, ServerError> {
    // ... implementation
}
```

### 7. Testing Your Changes

```bash
# Check for errors
cargo check

# Run clippy
cargo clippy --all-targets

# Build
cargo build

# Run tests
cargo test
```

### 8. Getting Help

If you encounter issues:
1. Check the `REFACTORING_SUMMARY.md` file
2. Look at existing features (e.g., `features/user/`) as examples
3. Compare with the import mapping above
4. Ask the team for help!

## Common Errors and Solutions

### Error: "unresolved import `crate::models`"
**Solution**: Update to use `crate::common::` or `crate::features::{feature}::models::`

### Error: "unresolved import `crate::api`"
**Solution**: Update to use `crate::features::{feature}::handlers::`

### Error: "unresolved import `crate::db`"
**Solution**: Update to use `crate::features::{feature}::repository::`

### Error: "SQLX_OFFLINE=true but there is no cached data"
**Solution**: Run `cargo sqlx prepare` with DATABASE_URL set

### Error: "failed to resolve: use of unresolved module `service`"
**Solution**: Update to use `crate::common::services::`
