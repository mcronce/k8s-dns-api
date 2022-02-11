#![allow(unused_parens)]
extern crate actix_web;
extern crate env_logger;
extern crate kube;

#[macro_use]
extern crate actix_helper_macros;

use std::convert::TryInto;
use std::env;
use std::error::Error;

use actix_web::web::Data;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::Api;
use kube::api::ListParams;

#[derive(Clone)]
struct State {
	service_tld: String,
	ingress_tld: String,
	client: kube::Client
}

/*
// TODO:  Figure out how to make this work since the OpenAPI types are all versioned
fn service_line<T>(service: &T, tld: &str, include_namespace: bool) -> Option<String> /* {{{ */ {
	let cluster_ip = match &service.spec.cluster_ip {
		None => return None,
		Some(s) => s
	};
	if(cluster_ip == "None") {
		return None;
	}
	if(include_namespace) {
		return format!("{} {}.{}{}", cluster_ip, service.metadata.name, service.metadata.namespace, tld);
	}
	format!("{} {}{}", cluster_ip, service.metadata.name, tld)
} // }}}
*/

async fn services_tuple(client: &kube::Client, tld: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> /* {{{ */ {
	let default_services = Api::<Service>::default_namespaced(client.clone()).list(&ListParams::default()).await?;
	let all_services = Api::<Service>::all(client.clone()).list(&ListParams::default()).await?;

	// Capacity here is just a hint; it'll probably be more than this, but it saves quite a few reallocations
	let mut lines = Vec::with_capacity(default_services.items.len() + all_services.items.len());

	for service in default_services.into_iter() {
		let name = match service.metadata.name.as_ref() {
			None => continue,
			Some(n) => n
		};
		let cluster_ip = match service.spec.and_then(|s| s.cluster_ip) {
			None => continue,
			Some(s) if s == "None" => continue,
			Some(s) => s
		};
		lines.push((cluster_ip, format!("{}{}", name, tld)));
	}

	for service in all_services.into_iter() {
		let name = match service.metadata.name.as_ref() {
			None => continue,
			Some(n) => n
		};
		let cluster_ip = match service.spec.and_then(|s| s.cluster_ip) {
			None => continue,
			Some(s) if s == "None" => continue,
			Some(s) => s
		};
		let namespace = match &service.metadata.namespace {
			None => "default",
			Some(s) if s.is_empty() => "default",
			Some(s) => s
		};
		lines.push((cluster_ip, format!("{}.{}{}", name, namespace, tld)));
	}

	Ok(lines)
} // }}}

#[responder]
async fn services(state: actix_web::web::Data<State>) -> actix_helper_macros::ResponderResult<()> {
	let lines: Vec<String> = services_tuple(&state.client, &state.service_tld).await?.iter().map(|t| format!("{} {}", t.0, t.1)).collect();
	Ok(actix_helper_macros::text!(lines.join("\n") + "\n"))
}

#[responder]
async fn services_unbound(state: actix_web::web::Data<State>) -> actix_helper_macros::ResponderResult<()> {
	let lines: Vec<String> = services_tuple(&state.client, &state.service_tld).await?.iter().map(|t| format!("local-data: \"{} 60 IN A {}\"", t.1, t.0)).collect();
	Ok(actix_helper_macros::text!(lines.join("\n") + "\n"))
}

#[derive(Debug)]
struct IngressNotFoundError; // {{{
impl std::fmt::Display for IngressNotFoundError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "Ingress not found")
	}
}
impl Error for IngressNotFoundError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		None
	}
}
// }}}

async fn ingresses_tuple(client: &kube::Client, tld: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> /* {{{ */ {
	// TODO:  Fix the absolutely hideous syntax in this function, especially that closure match
	let ingresses = Api::<Ingress>::all(client.clone());

	let ingress_service_ip = match {
		let list = Api::<Service>::namespaced(client.clone(), "kube-system").list(&ListParams::default()).await?;
		(|| {
			for service in list.into_iter() {
				let name = match service.metadata.name.as_ref() {
					Some(v) => v,
					None => continue
				};
				// TODO:  Allow configurable name
				if(name == "ingress-nginx-internal-controller") {
					match &service.spec.and_then(|s| s.cluster_ip) {
						None => return None,
						Some(s) => {
							if(s == "None") {
								return None
							} else {
								return Some(s.to_owned())
							}
						}
					};
				}
			}
			None
		})()
	} {
		None => return Err(Box::new(IngressNotFoundError)),
		Some(s) => s
	};

	let list = ingresses.list(&ListParams::default()).await?;
	// Capacity here is just a hint; we could have multiple hostnames per ingress, but this will save a lot of reallocations regardless
	let mut lines = Vec::with_capacity(list.items.len());
	for ingress in list.into_iter() {
		// TODO:  Allow configurable name
		match ingress.spec.as_ref().and_then(|s| s.ingress_class_name.as_ref()) {
			None => continue,
			Some(class) => if(class == "internal") {
				match ingress.spec.and_then(|s| s.rules) {
					None => continue,
					Some(rules) => for rule in rules.iter() {
						match &rule.host {
							None => continue,
							Some(host) => lines.push((ingress_service_ip.clone(), format!("{}{}", host, tld)))
						};
					}
				};
			}
		};
	}

	Ok(lines)
} // }}}

#[responder]
async fn ingresses(state: actix_web::web::Data<State>) -> actix_helper_macros::ResponderResult<()> {
	let lines: Vec<String> = ingresses_tuple(&state.client, &state.ingress_tld).await?.iter().map(|t| format!("{} {}", t.0, t.1)).collect();
	Ok(actix_helper_macros::text!(lines.join("\n") + "\n"))
}

#[responder]
async fn ingresses_unbound(state: actix_web::web::Data<State>) -> actix_helper_macros::ResponderResult<()> {
	let lines: Vec<String> = ingresses_tuple(&state.client, &state.ingress_tld).await?.iter().map(|t| format!("local-data: \"{} 60 IN A {}\"", t.1, t.0)).collect();
	Ok(actix_helper_macros::text!(lines.join("\n") + "\n"))
}

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let port = match env::var_os("NAMER_PORT") {
		Some(val) => val.into_string().unwrap().parse::<usize>().unwrap(),
		None => 80
	};
	let dir = match env::var_os("NAMER_STATIC_ROOT") {
		Some(val) => val.into_string().unwrap(),
		None => "/www".to_string()
	};
	let service_tld = match env::var_os("NAMER_SERVICE_TLD") {
		Some(val) => format!(".{}", val.into_string().unwrap()),
		None => "".to_string()
	};
	let ingress_tld = match env::var_os("NAMER_INGRESS_TLD") {
		Some(val) => format!(".{}", val.into_string().unwrap()),
		None => "".to_string()
	};

	env::set_var("RUST_LOG", "actix_web=info");
	env_logger::init();

	let client = {
		let mut config = kube::Config::infer().await.unwrap();
		config.default_namespace = "default".into();
		config.try_into().unwrap()
	};

	let state = State{
		service_tld,
		ingress_tld,
		client
	};

	let result = actix_web::HttpServer::new(move || actix_web::App::new()
		.app_data(Data::new(state.clone()))
		.wrap(actix_web::middleware::Logger::default())
		.route("/services.list", actix_web::web::get().to(services))
		.route("/unbound/services.list", actix_web::web::get().to(services_unbound))
		.route("/ingress-internal.list", actix_web::web::get().to(ingresses))
		.route("/unbound/ingress-internal.list", actix_web::web::get().to(ingresses_unbound))
		.service(actix_files::Files::new("/", &dir))
	).bind(format!("0.0.0.0:{}", port)).unwrap().run().await.unwrap();
	Ok(result)
}

