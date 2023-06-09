use hermes::{generate_code, redis::setex_key, respond, respond_with_body, send_mail};
use reqwest::{self, Client};
use serde::Deserialize;
use std::{env::var, format};
use vercel_runtime::{run, Body, Error, Request, Response, StatusCode};

#[derive(Deserialize)]
struct Payload {
    api_key: String,
    target: String,
    subject: String,
    app: String,
    ttl: Option<String>,
}

#[derive(Deserialize)]
struct App {
    name: String,
    owner: String,
    default_ttl: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

pub async fn handler(req: Request) -> Result<Response<Body>, Error> {
    match req.body() {
        Body::Binary(binary_body) => match String::from_utf8(binary_body.to_owned()) {
            Ok(string_body) => match serde_json::from_str::<Payload>(&string_body) {
                Ok(payload) => {
                    let api_key = var("SUPABASE_KEY").unwrap();
                    let db_url = var("SUPABASE_URL").unwrap();
                    let request = Client::new()
                        .get(format!(
                            "{db_url}/rest/v1/apps?id=eq.{}&owner=eq.{}",
                            &payload.app, &payload.api_key
                        ))
                        .header("apiKey", api_key)
                        .send()
                        .await;
                    match request {
                        Ok(response) => match response.status() {
                            StatusCode::OK => match response.text().await {
                                Ok(text) => match serde_json::from_str::<Vec<App>>(&text) {
                                    Ok(data) => {
                                        if data.len() == 0 {
                                            respond(StatusCode::UNAUTHORIZED)
                                        } else {
                                            let found_app = data.first().unwrap();
                                            let code = generate_code();
                                            let ttl: String;
                                            let text = format!(
                                                "<b>{}</b>",
                                                code
                                            );
                                            match payload.ttl {
                                                Some(specified_ttl) => {
                                                    ttl = specified_ttl;
                                                }
                                                None => {
                                                    ttl = found_app.default_ttl.to_owned();
                                                }
                                            }
                                            if setex_key(
                                                format!("code:{}:{}", &payload.app, &payload.target),
                                                code,
                                                ttl,
                                            )
                                            .await
                                            {
                                                if send_mail(payload.target, text, payload.subject)
                                                {
                                                    respond_with_body(
                                                        StatusCode::OK,
                                                        "Code sent to your user".to_string(),
                                                    )
                                                } else {
                                                    respond(StatusCode::INTERNAL_SERVER_ERROR)
                                                }
                                            } else {
                                                respond(StatusCode::INTERNAL_SERVER_ERROR)
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        respond_with_body(StatusCode::BAD_REQUEST, err.to_string())
                                    }
                                },
                                Err(_) => respond(StatusCode::INTERNAL_SERVER_ERROR),
                            },
                            _ => respond_with_body(StatusCode::BAD_REQUEST, "Here".to_string()),
                        },
                        Err(_) => respond(StatusCode::INTERNAL_SERVER_ERROR),
                    }
                }
                Err(err) => respond_with_body(StatusCode::BAD_REQUEST, err.to_string()),
            },
            Err(_) => respond(StatusCode::INTERNAL_SERVER_ERROR),
        },
        _ => respond(StatusCode::UNSUPPORTED_MEDIA_TYPE),
    }
}
