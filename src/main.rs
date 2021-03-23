use std::io::Write;

use actix_multipart::Multipart;
use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
use futures::{StreamExt, TryStreamExt};

async fn save_file(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition().unwrap();
        let filename = match content_disposition.get_filename() {
            Some(name) => name,
            None => continue // if filename is empty ignore it (filepond behaviour)
        };
        println!("{}", filename);
        
        let content_type: &mime::Mime = field.content_type();
        println!("content_type: {}", content_type);
        
        let extension = match (content_type.type_(), content_type.subtype()) {
            (mime::IMAGE, mime::JPEG) => "jpg",
            (mime::IMAGE, mime::PNG) => "png",
            _ => "other"
        };
        println!("extension: {}", extension);
        
        let filepath = format!("./tmp/{}", sanitize_filename::sanitize(&filename));


        // File::create is blocking operation, use threadpool
        let mut f = web::block(|| std::fs::File::create(filepath))
            .await
            .unwrap();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    Ok(HttpResponse::Ok().into())
}

fn index() -> HttpResponse {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form target="/" method="post" enctype="multipart/form-data">
                <input type="file" multiple name="file"/>
                <button type="submit">Submit</button>
            </form>
        </body>
    </html>"#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    std::fs::create_dir_all("./tmp").unwrap();

    let ip = "0.0.0.0:3000";

    HttpServer::new(|| {
        App::new().wrap(middleware::Logger::default())
        .service(
            web::resource("/")
                .route(web::get().to(index))
                .route(web::post().to(save_file)),
        )
        .service(
            web::resource("/api/images")
                .route(web::post().to(save_file)),
        )
    })
    .bind(ip)?
    .run()
    .await
}