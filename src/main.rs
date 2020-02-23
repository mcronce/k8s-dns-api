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

#[responder]
async fn services(state: actix_web::web::Data<State>) -> actix_helper_macros::ResponderResult<()> {
	// STARS, STARS everywhere.  Make it rain STARS.
	let services = kube::api::Api::v1Service((**state).client.clone());

	let default_services = services.clone().within("default").list(&kube::api::ListParams::default()).await?;
	let all_services = services.list(&kube::api::ListParams::default()).await?;

	let mut lines = Vec::with_capacity(default_services.items.len() + all_services.items.len());

	for service in default_services.iter() {
		let cluster_ip = match &service.spec.cluster_ip {
			None => continue,
			Some(s) => s
		};
		if(cluster_ip == "None") {
			continue;
		}
		lines.push(format!("{} {}{}", cluster_ip, service.metadata.name, state.tld));
	}

	for service in all_services.iter() {
		let cluster_ip = match &service.spec.cluster_ip {
			None => continue,
			Some(s) => s
		};
		if(cluster_ip == "None") {
			continue;
		}
		let mut namespace = match &service.metadata.namespace {
			None => "default",
			Some(s) => s
		};
		if(namespace == "") {
			namespace = "default";
		}
		lines.push(format!("{} {}.{}{}", cluster_ip, service.metadata.name, namespace, state.tld));
	}

	Ok(actix_helper_macros::text!(lines.join("\n") + "\n"))
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
		.route("/services.list", actix_web::web::get().to(services))
		.service(actix_files::Files::new("/", &dir))
	).bind(format!("0.0.0.0:{}", port))?.run().await?;
	Ok(result)
}

