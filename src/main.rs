use std::env;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use askama::{Template};
use axum::{extract::State, routing::get, Router};
use axum::http::StatusCode;
use sqlx::{Sqlite, SqlitePool};
use sqlx::migrate::MigrateDatabase;
use axum_client_ip::{SecureClientIp, SecureClientIpSource};
use sha256::{digest};
use tower_http::services::ServeDir;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexHtml {
    err: &'static str
}

#[derive(Template)]
#[template(path = "evaluation.html")]
struct EvaluationTemplate {}

async fn index(State(state): State<Arc<AppState>>, secure_ip: SecureClientIp) -> Result<IndexHtml, StatusCode> {
    let mut db = state.db_pool.acquire().await.unwrap();
    let time = chrono::offset::Utc::now();
    let iphash =
        digest(format!("{secure_ip:?}"));
    println!("{} => {iphash:?}", secure_ip.0);
    match sqlx::query!(
        r#"INSERT INTO feedback_staged_rollouts (date, ip_hash) VALUES (date(?1), ?2)"#,
        time, iphash
    )
        .execute(&mut *db)
        .await {
        Ok(_) => Ok(IndexHtml{err: ""}),
        Err(sqlx::Error::Database(err_box)) => {
            if err_box.as_ref().is_unique_violation() {
                Ok(IndexHtml{err: "Your ip has already been counted :)"})
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// async fn evaluation(State(state): State<Arc<AppState>>) -> Result<EvaluationTemplate, StatusCode> {
//
// }

async fn setup_db(db_url: &str, migrations_path: PathBuf) -> Result<sqlx::Pool<Sqlite>, sqlx::Error> {

    if !Sqlite::database_exists(db_url).await.unwrap_or(false) {
        println!("Creating database...");
        Sqlite::create_database(db_url).await?;
    }
    let db = SqlitePool::connect(db_url).await.unwrap();
    let migration_results = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .unwrap()
        .run(&db)
        .await;
    match migration_results {
        Ok(_) => println!("Migration success"),
        Err(error) => {
            panic!("error: {}", error);
        }
    }
    Ok(db)
}

struct AppState {
    db_pool: SqlitePool
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error>{
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let migrations = std::path::Path::new(&crate_dir).join("./migrations");
    let db_url = match env::var("DATABASE_URL") {
        Ok(val) => val,
        Err(_) => String::from("sqlite:memory")
    };
    let db = setup_db(&db_url, migrations).await?;

    let shared_state = Arc::new(AppState{
       db_pool: db
    });

    let app = Router::new()
        .route("/", get(index))
        .with_state(shared_state)
        .layer(SecureClientIpSource::ConnectInfo.into_extension())
        .nest_service("/static", ServeDir::new(PathBuf::from("static")));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
    Ok(())
}
