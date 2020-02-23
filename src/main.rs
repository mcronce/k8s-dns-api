#![allow(unused_parens)]
extern crate actix_web;
extern crate env_logger;
extern crate kube;

#[macro_use]
extern crate actix_helper_macros;

use std::env;
use std::sync;
use std::error::Error;

#[derive(Clone)]
struct State {
	tld: String,
	client: kube::client::APIClient
}

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
	let tld = match env::var_os("NAMER_TLD") {
		Some(val) => match val.into_string() {
			Ok(v) => format!(".{}", v),
			Err(e) => panic!(e)
		},
		None => "".to_string()
	};

	env::set_var("RUST_LOG", "actix_web=info");
	env_logger::init();

	let client = {
		let k8s_config = kube::config::load_kube_config().await?;
		kube::client::APIClient::new(k8s_config)
	};

	let state = State{
		tld: tld,
		client: client
	};

	let result = actix_web::HttpServer::new(move || actix_web::App::new()
		.data(state.clone())
		.wrap(actix_web::middleware::Logger::default())
		.service(actix_files::Files::new("/", &dir))
	).bind(format!("0.0.0.0:{}", port))?.run().await?;
	Ok(result)
}

