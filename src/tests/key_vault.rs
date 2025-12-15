#[cfg(test)]
mod tests {
    use std::{env, sync::Arc};

    use dotenv::dotenv;

    use crate::{
        models::{app_state::AppState, game_base::GameType},
        service::key_vault::KeyVaultError,
    };

    async fn setup_app_state() -> Arc<AppState> {
        dotenv().ok();
        let connection_string =
            env::var("TERO__DATABASE_URL").expect("Failed to obtain connection string");
        AppState::from_connection_string(&connection_string)
            .await
            .unwrap()
    }

    #[allow(clippy::assertions_on_constants)]
    #[tokio::test]
    async fn max_limit_keys() {
        let state = setup_app_state().await;
        let vault = state.get_vault();

        for num in 0..10_000 {
            let word = vault.create_key(state.get_pool(), GameType::Quiz).unwrap();
            println!("{} - {}", num + 1, word)
        }

        let result = vault.create_key(state.get_pool(), GameType::Quiz);
        assert!(result.is_err());

        let error = result.err().unwrap();
        match error {
            KeyVaultError::FullCapasity => assert!(true),
            _ => assert!(false, "Failed with: {}", error),
        }
    }

    #[tokio::test]
    async fn test_concurrent_key_creation() {
        let state = setup_app_state().await;

        let mut handles = Vec::new();

        for i in 0..10001 {
            let state_clone = Arc::clone(&state);

            let handle = tokio::spawn(async move {
                let vault = state_clone.get_vault();
                match vault.create_key(state_clone.get_pool(), GameType::Quiz) {
                    Ok(key) => {
                        println!("Task {} opprettet nøkkel: {}", i, key);
                        Ok(key)
                    }
                    Err(e) => {
                        println!("Task {} feilet: {:?}", i, e);
                        Err(e)
                    }
                }
            });

            handles.push(handle);
        }

        // Await alle tasks samtidig
        let results = futures::future::join_all(handles).await;

        // Analyser resultater
        let mut successful_keys: Vec<String> = Vec::new();
        let mut failed_count = 0;

        for result in results {
            match result.unwrap() {
                // unwrap the JoinResult
                Ok(key) => successful_keys.push(key),
                Err(_) => failed_count += 1,
            }
        }

        println!("Vellykkede nøkler: {}", successful_keys.len());
        println!("Feilede forsøk: {}", failed_count);

        // Sjekk at alle nøkler er unike
        let mut unique_keys = std::collections::HashSet::new();
        for key in &successful_keys {
            assert!(
                unique_keys.insert(key.clone()),
                "Duplikat nøkkel funnet: {}",
                key
            );
        }

        assert!(successful_keys.is_empty(), "Ingen nøkler ble opprettet");
        assert_eq!(
            successful_keys.len(),
            unique_keys.len(),
            "Duplikate nøkler oppdaget"
        );
    }
}
