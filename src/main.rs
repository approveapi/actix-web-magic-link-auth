use env_logger;
use actix_web::{server, middleware,
                App, Query, Form, HttpRequest, HttpResponse,
                AsyncResponder, ResponseError, Responder, Result};

use actix_web::http::{StatusCode, Method, header};
use middleware::session::{CookieSessionBackend, SessionStorage, RequestSession};

use futures::prelude::*;
use serde::{Serialize, Deserialize};
use rand::rngs::{OsRng};
use rand::RngCore;
use base_62;
use base64;

use approveapi::*;

use std::env;
use failure::Fail;

fn home(request: HttpRequest) -> impl Responder {
    let authenticated_user:Option<String> = request.session().get("authenticated_user").unwrap_or(None);

    // if we have a logged in user
    if let Some(user) = authenticated_user {
        let home = include_str!("../static/home.html");
        let home = home.replace("USER_ID", &user);
        return HttpResponse::build(StatusCode::OK)
            .content_type("text/html; charset=utf-8")
            .body(home);
    }

    // else not logged in, redirect to /login page!
    HttpResponse::build(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, "/login")
        .finish()
}

fn login_page(_: HttpRequest) -> Result<HttpResponse> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/login.html")))
}


#[derive(Deserialize)]
struct LoginForm {
    user: String, // an email address or phone
}

fn random_challenge() -> String {
    let mut nonce = vec![0u8; 32];
    OsRng::new().unwrap().fill_bytes(&mut nonce);
    base_62::encode(&nonce)
}

#[derive(Serialize, Deserialize)]
struct LoginChallenge {
    user: String,
    challenge: String,
}

#[derive(Debug, Fail)]
#[fail(display = "Internal Server Error Occured")]
struct ServerError(String);
impl ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(StatusCode::BAD_REQUEST)
            .content_type("text/html; charset=utf-8")
            .body(format!("ERROR! {}", self.0))
    }
}

fn login_submit((form, request): (Form<LoginForm>, HttpRequest)) -> impl Responder {
    // the user we are verifying
    let user = form.into_inner().user;
    eprintln!("got user: {:?}", user);

    // create a new challenge for this user
    let challenge = random_challenge();

    // save the challenge <-> user mapping
    let _ = request.session().set("pending_login_challenge", LoginChallenge {
        user: user.clone(),
        challenge: challenge.clone(),
    });
    eprintln!("set session");

    // create the prompt for magic sign-in approval
    let client = approveapi::create_client(env::var("APPROVEAPI_TEST_KEY").expect("Missing env ApproveAPI Test API Key"));
    let mut prompt_request = CreatePromptRequest::new(
        user,
        r#"Click the link below to sign in to your account.
        This link will expire in 24 hours."#.to_string(),
    );
    prompt_request.title = Some("Magic sign-in link".to_string());
    prompt_request.approve_text = Some("Sign-in".to_string());
    prompt_request.approve_redirect_url = Some(format!("{}/verify_login?c={}",
                                                       env::var("WEB_URL").unwrap_or("http://localhost:5000".to_string()),
                                                       challenge));
    //todo: add time, ip address, etc.

    //todo: handle errors from approveapi
    client.create_prompt(prompt_request).map_err(|e| {
        eprintln!("approveapi error: {:?}", e);
        ServerError(format!("approveapi error: {:?}", e))
    }).and_then(|_| {
        Ok(HttpResponse::build(StatusCode::OK)
            .content_type("text/html; charset=utf-8")
            .body(include_str!("../static/challenge.html")))

    }).responder()
}

#[derive(Deserialize)]
struct VerifyLoginQuery {
    #[serde(rename = "c")]
    challenge: String,
}

fn verify_login((query, request): (Query<VerifyLoginQuery>, HttpRequest)) -> Result<HttpResponse> {
    let pending_challenge: Option<LoginChallenge> = request.session().get("pending_login_challenge")?;

    let login_challenge = match pending_challenge {
        Some(lc) => lc,
        None => {
            // Couldn't parse the session, so return an error
            return Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
                .content_type("text/html; charset=utf-8")
                .body("Invalid session"));
        }
    };

    if login_challenge.challenge == query.into_inner().challenge {
        // save the authenticated user
        let _ = request.session().set("authenticated_user", login_challenge.user);

        // redirect to home
        Ok(HttpResponse::build(StatusCode::TEMPORARY_REDIRECT)
            .header(header::LOCATION, "/")
            .finish())

    } else {
        // the challenge does not match!
        Ok(HttpResponse::build(StatusCode::UNAUTHORIZED)
            .content_type("text/html; charset=utf-8")
            .body("Invalid challenge"))
    }
}

fn main() {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let cookie_secret = base64::decode(&env::var("COOKIE_SECRET_KEY").expect("Missing env cookie secret key.")).unwrap();
    let _ = env::var("APPROVEAPI_TEST_KEY").expect("Missing env ApproveAPI Test API Key");

    let is_prod = env::var("PRODUCTION").is_ok();

    server::new(
        move || {
            App::new()
                .middleware(middleware::Logger::default())
                .middleware(SessionStorage::new(
                    // Important note: cookie session MUST be PRIVATE in this example because the challenge,
                    // which must be kept secret when authentication is pending, is store in the cookie
                    // to make this example simple.
                    CookieSessionBackend::private(&cookie_secret).secure(is_prod == true)
                ))
                .route("/", Method::GET, home)
                .route("/login", Method::GET, login_page)
                .route("/login", Method::POST, login_submit)
                .route("/verify_login", Method::GET, verify_login)
        })
        .bind("0.0.0.0:5000").unwrap()
        .run()
}