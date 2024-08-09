use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use std::io::Error;
use std::net::TcpListener;

async fn greet(req: HttpRequest) -> HttpResponse {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", name);
    HttpResponse::Ok().body(format!("Hello {}!", name))
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
pub fn run(listener: TcpListener) -> Result<Server, Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/", web::get().to(greet))
            .route("/{name}", web::get().to(greet))
    })
        .listen(listener)?
        .run();

    Ok(server)
}
