pub(crate) mod config;
pub(crate) mod data;
pub(crate) mod error;
pub(crate) mod handle;

use self::data::AppData;
use crate::db::StoreDB;

use actix_web::web;
use chrono::Local;
use env_logger::fmt::Color;
use sea_orm::Database;
use std::io::Write;

pub type BoxedResult<T> = Result<T, Box<dyn std::error::Error>>;
pub type BoxedSyncResult<T> = Result<T, Box<dyn std::error::Error + Sync + Send>>;

pub fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let mut style = buf.style();
            let prefix = style.set_color(Color::Black).set_intense(true).value("[");
            let mut style = buf.style();
            let suffix = style.set_color(Color::Black).set_intense(true).value("]");
            writeln!(
                buf,
                "{}{} {:<5} {}{} {}",
                prefix,
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                buf.default_styled_level(record.level()),
                record.module_path().unwrap_or_default(),
                suffix,
                record.args()
            )
        })
        .init();
}

pub async fn init_app_data(url: &str, chain_id: u32) -> BoxedResult<web::Data<AppData>> {
    let pool = Database::connect(url).await?;
    let store_db = StoreDB::init_db(pool, chain_id).await?;

    let categories = vec![
        "Cryptocurrency",
        "Politics",
        "Arts",
        "Business & Finance",
        "Sport",
        "Climate",
        "Disaster",
        "Other",
    ];

    let liquidity = vec![
        "24h volume",
        "48h volume",
        "Total volume",
        "Newest",
        "Closing soon",
    ];
    let proposals = store_db.read_proposals().await?;
    let app_data = AppData::new(store_db.clone(), categories, liquidity, chain_id, proposals);
    let data = web::Data::new(app_data);

    let list = store_db.read_coins_support().await?;
    for (addr, symbol, flag) in list.iter() {
        data.insert_support(addr.to_string(), symbol.to_string(), *flag)?;
    }
    Ok(data)
}
