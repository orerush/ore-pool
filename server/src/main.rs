mod aggregator;
mod contributor;
mod database;
mod error;
mod operator;
mod tx;
mod utils;

use core::panic;
use std::str::FromStr;

use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use aggregator::{Aggregator, Contribution};
use operator::Operator;
use solana_sdk::pubkey::Pubkey;
use utils::create_cors;

// TODO: publish attestation to s3
// write attestation url to db with last-hash-at as foreign key
#[actix_web::main]
async fn main() -> Result<(), error::Error> {
    env_logger::init();
    // operator and aggregator mutex
    let operator = web::Data::new(Operator::new()?);
    let aggregator = tokio::sync::RwLock::new(Aggregator::new(&operator).await?);
    let aggregator = web::Data::new(aggregator);
    // contributions async channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Contribution>();
    let tx = web::Data::new(tx);
    // env vars
    let attribution_epoch = attribution_epoch()?;
    let stake_commit_epoch = stake_commit_epoch()?;
    let boosts = load_boosts()?;

    // aggregate contributions
    tokio::task::spawn({
        let operator = operator.clone();
        let aggregator = aggregator.clone();
        async move {
            if let Err(err) =
                aggregator::process_contributions(aggregator.as_ref(), operator.as_ref(), &mut rx)
                    .await
            {
                log::error!("{:?}", err);
            }
        }
    });

    // kick off attribution loop
    tokio::task::spawn({
        let operator = operator.clone();
        async move {
            loop {
                // submit attributions
                let operator = operator.clone().into_inner();
                if let Err(err) = operator.attribute_members().await {
                    panic!("{:?}", err)
                }
                // sleep until next epoch
                tokio::time::sleep(tokio::time::Duration::from_secs(60 * attribution_epoch)).await;
            }
        }
    });

    // kick off commit-stake loop
    tokio::task::spawn({
        let operator = operator.clone();
        async move {
            if boosts.len().gt(&0) {
                loop {
                    let operator = operator.clone().into_inner();
                    if let Err(err) = operator.commit_stake(boosts.as_slice()).await {
                        panic!("{:?}", err)
                    }
                    // sleep until next epoch
                    tokio::time::sleep(tokio::time::Duration::from_secs(60 * stake_commit_epoch))
                        .await;
                }
            }
        }
    });

    // launch server
    HttpServer::new(move || {
        log::info!("starting server");
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(create_cors())
            .app_data(tx.clone())
            .app_data(operator.clone())
            .app_data(aggregator.clone())
            .service(web::resource("/member/{authority}").route(web::get().to(contributor::member)))
            .service(web::resource("/pool-address").route(web::get().to(contributor::pool_address)))
            .service(web::resource("/register").route(web::post().to(contributor::register)))
            .service(web::resource("/contribute").route(web::post().to(contributor::contribute)))
            .service(web::resource("/challenge").route(web::get().to(contributor::challenge)))
            .service(
                web::resource("/update-balance").route(web::post().to(contributor::update_balance)),
            )
            .service(health)
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
    .map_err(From::from)
}

fn load_boosts() -> Result<Vec<Pubkey>, error::Error> {
    let boost_1 = boost_one()?;
    let boost_2 = boost_two()?;
    let boost_3 = boost_three()?;
    let boosts: Vec<Pubkey> = vec![boost_1, boost_2, boost_3]
        .into_iter()
        .flatten()
        .collect();
    Ok(boosts)
}

fn load_boost(var: String) -> Result<Option<Pubkey>, error::Error> {
    match std::env::var(var) {
        Ok(boost) => {
            let boost = Pubkey::from_str(boost.as_str())?;
            Ok(Some(boost))
        }
        // optional
        Err(_) => Ok(None),
    }
}

fn boost_one() -> Result<Option<Pubkey>, error::Error> {
    load_boost("BOOST_ONE".to_string())
}

fn boost_two() -> Result<Option<Pubkey>, error::Error> {
    load_boost("BOOST_TWO".to_string())
}

fn boost_three() -> Result<Option<Pubkey>, error::Error> {
    load_boost("BOOST_THREE".to_string())
}

// denominated in minutes
fn stake_commit_epoch() -> Result<u64, error::Error> {
    let string = std::env::var("STAKE_EPOCH")?;
    let epoch: u64 = string.parse()?;
    Ok(epoch)
}

// denominated in minutes
fn attribution_epoch() -> Result<u64, error::Error> {
    let string = std::env::var("ATTR_EPOCH")?;
    let epoch: u64 = string.parse()?;
    Ok(epoch)
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("ok")
}
