use actix_files::Files as Fs;
use actix_web::{
    error, get, middleware, post, web, App, Error, HttpRequest, HttpResponse, HttpServer, Result,
};

use entity::post;
use entity::post::Entity as Post;
use listenfd::ListenFd;
use migration::{Migrator, MigratorTrait};
use sea_orm::{DatabaseConnection, TransactionStream};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Mutex;
use actix_files::FilesService;
use actix_web::cookie::time::macros::date;
use actix_web::web::{Data, Form};
use tera::Tera;

const DEFAULT_POSTS_PER_PAGE: usize = 5;

#[derive(Debug, Clone)]
struct AppState {
    templates: tera::Tera,
    conn: DatabaseConnection,
}

#[derive(Debug, Deserialize)]
pub struct Params {
    page: Option<usize>,
    posts_per_page: Option<usize>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct FlashData {
    kind: String,
    message: String,
}

#[get("/")]
async fn list(req: HttpRequest, data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let template = &data.templates;
    let conn = &data.conn;

    let params = web::Query::<Params>::from_query(req.query_string()).unwrap();
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.posts_per_page.unwrap_or(DEFAULT_POSTS_PER_PAGE);
    let paginator = Post::find()
        .order_by_asc(post::Column::Id)
        .paginate(conn, posts_per_page);
    let num_pages = paginator.num_pages().await.ok().unwrap();

    let posts = paginator
        .fetch_page(page - 1)
        .await
        .expect("could not retrieve posts");
    let mut ctx = tera::Context::new();
    ctx.insert("posts", &posts);
    ctx.insert("page", &page);
    ctx.insert("posts_per_page", &posts_per_page);
    ctx.insert("num_pages", &num_pages);

    let body = template
        .render("index.html.tera", &ctx)
        .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[get("/new")]
async fn new(data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let template = &data.templates;
    let ctx = tera::Context::new();
    let body = template.render("new.html.tera", &ctx)
        .map_err(|_| error::ErrorInternalServerError("templdate error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("/")]
async fn create(data: Data<AppState>, post_form: Form<post::Model>) -> Result<HttpResponse, Error> {
    let conn = &data.conn;
    let form = post_form.into_inner();
    post::ActiveModel {
        title: Set(form.title.to_owned()),
        text: Set(form.text.to_owned()),
        ..Default::default()
    }
        .save(conn)
        .await
        .expect("could not insert post");
    Ok(HttpResponse::Found().append_header(("location", "/")).finish())
}


fn get_env_var(str: &str) -> String {
    let string = format!("{} is not set in .env file", str);
    env::var(str).expect(&*string)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "DEBUG");
    tracing_subscriber::fmt::init();

    dotenv::dotenv().ok();
    let db_url = get_env_var("DATABASE_URL");
    let host = get_env_var("HOST");
    let port = get_env_var("PORT");
    let server_url = format!("{}:{}", host, port);
    let conn = sea_orm::Database::connect(&db_url).await.unwrap();
    Migrator::up(&conn, None).await.unwrap();

    let templates = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();
    let state = AppState { templates, conn };

    let mut listenfd = ListenFd::from_env();
    let mut server = HttpServer::new(move || {
        App::new()
            .service(Fs::new("/static", "./static"))
            .app_data(web::Data::new(state.clone()))
            .wrap(middleware::Logger::default())
            .configure(init)
    });
    server = match listenfd.take_tcp_listener(0)? {
        Some(listener) => server.listen(listener)?,
        None => server.bind(&server_url)?
    };
    println!("start server at {}", server_url);
    server.run().await?;
    Ok(())
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(list);
    cfg.service(new);
    cfg.service(create);
}