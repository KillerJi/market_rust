mod actors;
mod datas;
mod db;
mod entity;
mod xprotocol;

use crate::{actors::block::BlockActor, datas::handle::Handlers};
use actix::Actor;
use actix_web::{self, middleware::Logger, App, HttpServer};
use dotenv::dotenv;
#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    datas::init_logger();

    let config = datas::config::Config::from_env()?;

    let bind_address = format!("{}:{}", config.server.host, config.server.port);

    let data = datas::init_app_data(&config.server.database_url, config.contract.chain_id).await?;
    BlockActor::new((*data).clone(), &config.contract)?.start();

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .configure(Handlers::app_config)
            .wrap(Logger::default())
    })
    .workers(num_cpus::get())
    .bind(bind_address)?
    .run()
    .await?;
    Ok(())
}
