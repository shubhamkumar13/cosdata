use actix_cors::Cors;
use actix_web::{
    dev::ServiceRequest, middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_httpauth::{extractors::bearer::BearerAuth, middleware::HttpAuthentication};
use rustls::{pki_types::PrivateKeyDer, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};

use crate::api;
use std::env;

#[derive(Debug, Serialize, Deserialize)]
struct MyObj {
    name: String,
    number: i32,
}

async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // println!("cred: {credentials:?}");
    Ok(req)
}

async fn extract_item(item: web::Json<MyObj>, req: HttpRequest) -> HttpResponse {
    // println!("request: {req:?}");
    // println!("model: {item:?}");

    HttpResponse::Ok().json(item.0) // <- send json response
}

/// This handler manually load request payload and parse json object
async fn index_manual(body: web::Bytes) -> Result<HttpResponse, Error> {
    // body is loaded, now we can deserialize serde-json
    let obj = serde_json::from_slice::<MyObj>(&body)?;
    Ok(HttpResponse::Ok().json(obj)) // <- send response
}

#[actix_web::main]
pub async fn run_actix_server() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let config = load_rustls_config();

    log::info!("starting HTTPS server at https://localhost:8443");

    HttpServer::new(move || {
        let auth = HttpAuthentication::bearer(validator);
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            // ensure the CORS middleware is wrapped around the httpauth middleware
            // so it is able to add headers to error responses
            .wrap(Cors::permissive())
            // register simple handler, handle all methods
            .app_data(web::JsonConfig::default().limit(4 * 1048576))
            .route(
                "/auth/gettoken",
                web::post().to(crate::api::auth::get_token),
            )
            .service(
                web::scope("/vectordb")
                    .wrap(auth.clone())
                    .service(
                        web::resource("/createdb").route(web::post().to(api::vectordb::create)),
                    )
                    .service(web::resource("/upsert").route(web::post().to(api::vectordb::upsert)))
                    .service(web::resource("/search").route(web::post().to(api::vectordb::search)))
                    .service(web::resource("/fetch").route(web::post().to(api::vectordb::fetch)))
                    .service(
                        web::scope("{database_name}/transactions")
                            .route("/", web::post().to(api::vectordb::transactions::create))
                            .route(
                                "/{transaction_id}/upsert",
                                web::post().to(api::vectordb::transactions::upsert),
                            )
                            .route(
                                "/{transaction_id}/update",
                                web::post().to(api::vectordb::transactions::update),
                            )
                            .route(
                                "/{transaction_id}/delete",
                                web::post().to(api::vectordb::transactions::delete),
                            )
                            .route(
                                "/{transaction_id}/commit",
                                web::post().to(api::vectordb::transactions::commit),
                            )
                            .route(
                                "/{transaction_id}/abort",
                                web::post().to(api::vectordb::transactions::abort),
                            ),
                    ),
            )
        // .service(web::resource("/index").route(web::post().to(index)))
        // .service(
        //     web::resource("/extractor")
        //         .app_data(web::JsonConfig::default().limit(1024))
        // <- limit size of the payload (resource level)
        //         .route(web::post().to(extract_item)),
        // )
        // .service(web::resource("/manual").route(web::post().to(index_manual)))
        // .service(web::resource("/").route(web::post().to(index)))
    })
    .bind_rustls_0_23("127.0.0.1:8443", config)?
    .run()
    .await
}

fn load_rustls_config() -> rustls::ServerConfig {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    // init server config builder with safe defaults
    let mut config = ServerConfig::builder().with_no_client_auth();

    let key = "SSL_CERT_DIR";
    let ssl_cert_dir = match env::var_os(key) {
        Some(val) => val.into_string().unwrap_or_else(|_| {
            eprintln!("{key} is not a valid UTF-8 string.");
            std::process::exit(1);
        }),
        None => {
            eprintln!("{key} is not defined in the environment.");
            std::process::exit(1);
        }
    };

    let cert_file_path = format!("{}/certs/cosdata-ssl.crt", ssl_cert_dir);
    let key_file_path = format!("{}/private/cosdata-ssl.key", ssl_cert_dir);

    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open(&cert_file_path).unwrap_or_else(|_| {
        eprintln!("Failed to open certificate file: {}", cert_file_path);
        std::process::exit(1);
    }));
    let key_file = &mut BufReader::new(File::open(&key_file_path).unwrap_or_else(|_| {
        eprintln!("Failed to open key file: {}", key_file_path);
        std::process::exit(1);
    }));

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("Failed to parse certificate chain.");
            std::process::exit(1);
        });
    let mut keys = pkcs8_private_keys(key_file)
        .map(|key| key.map(PrivateKeyDer::Pkcs8))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|_| {
            eprintln!("Failed to parse private keys.");
            std::process::exit(1);
        });

    // exit if no keys could be parsed
    if keys.is_empty() {
        eprintln!("Could not locate PKCS 8 private keys.");
        std::process::exit(1);
    }

    config.with_single_cert(cert_chain, keys.remove(0)).unwrap()
}

fn old_load_rustls_config() -> rustls::ServerConfig {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    // init server config builder with safe defaults
    let config = ServerConfig::builder().with_no_client_auth();

    let key = "SSL_CERT_DIR";
    match env::var_os(key) {
        Some(val) => println!("{key}: {val:?}"),
        None => println!("{key} is not defined in the environment."),
    }
    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open("~/example.crt").unwrap());
    let key_file = &mut BufReader::new(File::open("~/example.key").unwrap());

    // convert files to key/cert objects
    let cert_chain = certs(cert_file).collect::<Result<Vec<_>, _>>().unwrap();
    let mut keys = pkcs8_private_keys(key_file)
        .map(|key| key.map(PrivateKeyDer::Pkcs8))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // exit if no keys could be parsed
    if keys.is_empty() {
        eprintln!("Could not locate PKCS 8 private keys.");
        std::process::exit(1);
    }

    config.with_single_cert(cert_chain, keys.remove(0)).unwrap()
}
