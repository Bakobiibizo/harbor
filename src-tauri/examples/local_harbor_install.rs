//! Local cross-repository acceptance harness for the Neo Grounds package handoff.
//!
//! This example deliberately exposes no Tauri command or user-facing store surface. It opens one
//! explicitly selected disposable profile and invokes the same HTTP/validation/install path used
//! by Harbor's production command.

use harbor_lib::{
    commands::game_library::install_store_game_from_origin, db::Database,
    services::GameLibraryService,
};
use std::{env, path::PathBuf, process::ExitCode, sync::Arc};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("serializable result")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or("expected install or list command")?;
    let profile_root = PathBuf::from(args.next().ok_or("expected profile root")?);
    let database = Arc::new(Database::new(profile_root.join("harbor.db"))?);
    let service = Arc::new(GameLibraryService::new(&profile_root, database)?);

    match command.as_str() {
        "install" => {
            let origin = args.next().ok_or("expected store origin")?;
            let game_id = args.next().ok_or("expected game ID")?;
            let version_id = args.next().ok_or("expected version ID")?;
            let permissions: Vec<String> =
                serde_json::from_str(&args.next().ok_or("expected permissions JSON")?)?;
            reject_extra(args)?;
            let installation = install_store_game_from_origin(
                &origin,
                &game_id,
                &version_id,
                &permissions,
                service,
                chrono::Utc::now().timestamp(),
            )
            .await?;
            Ok(serde_json::to_value(installation)?)
        }
        "list" => {
            reject_extra(args)?;
            Ok(serde_json::to_value(service.list()?)?)
        }
        _ => Err("expected install or list command".into()),
    }
}

fn reject_extra(mut args: impl Iterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    if args.next().is_some() {
        return Err("received unexpected extra arguments".into());
    }
    Ok(())
}
