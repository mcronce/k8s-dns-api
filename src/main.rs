extern crate actix_web;
extern crate env_logger;

use std::env;
use std::error::Error;

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let port = match env::var_os("NAMER_PORT") {
		Some(val) => match val.into_string() {
			Ok(v) => v,
			Err(e) => panic!(e)
		}.parse::<usize>()?,
		None => 80
	};
	let dir = match env::var_os("NAMER_STATIC_ROOT") {
		Some(val) => match val.into_string() {
			Ok(v) => v,
			Err(e) => panic!(e)
		},
		None => "/www".to_string()
	};

	env::set_var("RUST_LOG", "actix_web=info");
	env_logger::init();

	let result = actix_web::HttpServer::new(move || actix_web::App::new()
		.wrap(actix_web::middleware::Logger::default())
		.service(actix_files::Files::new("/", &dir))
	).bind(format!("0.0.0.0:{}", port))?.run().await?;
	Ok(result)
}

