use std::io::Write;
use uuid::Uuid;
use std::path::Path;

use actix_multipart::Multipart;
use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
use futures::{StreamExt, TryStreamExt};
const IMAGE_PATH: &str = "/var/www/html/images";


fn get_filename(original_filename: &str, extension: String) -> String {
    let sanitized = sanitize_filename::sanitize(&original_filename);
    let filename_without_extension = Path::new(&sanitized).file_stem().unwrap().to_str().unwrap();
    let mut full_path = format!("{}/{}.{}", IMAGE_PATH, filename_without_extension, extension);
    let mut filename = format!("{}.{}", filename_without_extension, extension);
    
    let mut count: i8 = 1;
    while Path::new(&full_path).exists() {
        full_path = format!("{}/{}-{}.{}", IMAGE_PATH, filename_without_extension, count, extension);
        filename = format!("{}-{}.{}", filename_without_extension, count, extension);
        count += 1;
    }

    filename
}

async fn save_file(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // let uuid = Uuid::new_v4().to_hyphenated().to_string();
    let mut filename: Option<String> = None;
    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition().unwrap();
        let original_filename = match content_disposition.get_filename() {
            Some(name) => name,
            None => continue, // if filename is empty ignore it (filepond behaviour)
        };
        println!("{}", original_filename);

        let content_type: &mime::Mime = field.content_type();
        println!("content_type: {}", content_type);

        let extension = match (content_type.type_(), content_type.subtype()) {
            (mime::IMAGE, mime::JPEG) => String::from("jpg"),
            (mime::IMAGE, mime::PNG) => String::from("png"),
            _ => return Ok(HttpResponse::UnsupportedMediaType().into()),
        };
        println!("extension: {}", extension);

        let name = get_filename(original_filename, extension);
        let filepath = format!(
            "{}/{}",
            IMAGE_PATH,
            sanitize_filename::sanitize(&name)
        );

        filename = Some(name);
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

    match filename {
        Some(file) => {
            return Ok(HttpResponse::Created()
                .content_type("text/html; charset=utf-8")
                .body(format!("http://localhost/images/{}", file)))
        }
        None => return Ok(HttpResponse::BadRequest().into()),
    }
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
    std::fs::create_dir_all(IMAGE_PATH).unwrap();

    let ip = "0.0.0.0:3333";

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/")
                    .route(web::get().to(index))
                    .route(web::post().to(save_file)),
            )
            .service(web::resource("/images").route(web::post().to(save_file)))
    })
    .bind(ip)?
    .run()
    .await
}
