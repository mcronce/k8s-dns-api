#![feature(never_type)]
#![allow(unused_parens)]
use std::convert::TryInto;
use std::env;
use std::iter;

use futures::stream;
use futures::StreamExt;

use actix_web::web::Data;
use actix_web::HttpResponse;
use compact_str::format_compact;
use compact_str::CompactString;
use kube::Client;

mod error;
use error::Error;
mod services_stream;
use services_stream::ServiceStream;
mod ingresses_stream;
use ingresses_stream::IngressStream;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[repr(transparent)]
struct ServiceTld(CompactString);
#[repr(transparent)]
struct IngressTld(CompactString);

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

async fn services(client: Data<Client>, service_tld: Data<ServiceTld>) -> Result<HttpResponse, Error> {
	let stream = ServiceStream::new(&client, &service_tld.0)
		.await?
		.map(|r| r.map(|t| format!("{} {}\n", t.0, t.1).into()))
		.chain(stream::iter(iter::once(Ok("\n".into()))));
	Ok(HttpResponse::Ok().content_type("text/plain").streaming(stream))
}

async fn services_unbound(client: Data<Client>, service_tld: Data<ServiceTld>) -> Result<HttpResponse, Error> {
	let stream = ServiceStream::new(&client, &service_tld.0)
		.await?
		.map(|r| r.map(|t| format!("local-data: \"{} 60 IN A {}\"\n", t.1, t.0).into()))
		.chain(stream::iter(iter::once(Ok("\n".into()))));
	Ok(HttpResponse::Ok().content_type("text/plain").streaming(stream))
}

async fn ingresses(client: Data<Client>, ingress_tld: Data<IngressTld>) -> Result<HttpResponse, Error> {
	let stream = IngressStream::new(&client, &ingress_tld.0)
		.await?
		.map(|r| r.map(|t| format!("{} {}\n", t.0, t.1).into()))
		.chain(stream::iter(iter::once(Ok("\n".into()))));
	Ok(HttpResponse::Ok().content_type("text/plain").streaming(stream))
}

async fn ingresses_unbound(client: Data<Client>, ingress_tld: Data<IngressTld>) -> Result<HttpResponse, Error> {
	let stream = IngressStream::new(&client, &ingress_tld.0)
		.await?
		.map(|r| r.map(|t| format!("local-data: \"{} 60 IN A {}\"\n", t.1, t.0).into()))
		.chain(stream::iter(iter::once(Ok("\n".into()))));
	Ok(HttpResponse::Ok().content_type("text/plain").streaming(stream))
}

#[actix_web::main]
async fn main() {
	let port = match env::var_os("NAMER_PORT") {
		Some(val) => val.into_string().unwrap().parse::<usize>().unwrap(),
		None => 80
	};
	let dir = match env::var_os("NAMER_STATIC_ROOT") {
		Some(val) => val.into_string().unwrap(),
		None => "/www".to_string()
	};
	let service_tld = ServiceTld(env::var_os("NAMER_SERVICE_TLD").map(|s| format_compact!(".{}", s.into_string().unwrap())).unwrap_or_default());
	let ingress_tld = IngressTld(env::var_os("NAMER_INGRESS_TLD").map(|s| format_compact!(".{}", s.into_string().unwrap())).unwrap_or_default());

	env::set_var("RUST_LOG", "actix_web=info");
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.compact()
		.init();

	let client: Client = {
		let mut config = kube::Config::infer().await.unwrap();
		config.default_namespace = "default".into();
		config.try_into().unwrap()
	};

	let service_tld = Data::new(service_tld);
	let ingress_tld = Data::new(ingress_tld);
	let client = Data::new(client);

	actix_web::HttpServer::new(move || actix_web::App::new()
		.app_data(service_tld.clone())
		.app_data(ingress_tld.clone())
		.app_data(client.clone())
		.wrap(actix_web::middleware::Logger::default())
		.route("/services.list", actix_web::web::get().to(services))
		.route("/unbound/services.list", actix_web::web::get().to(services_unbound))
		.route("/ingress-internal.list", actix_web::web::get().to(ingresses))
		.route("/unbound/ingress-internal.list", actix_web::web::get().to(ingresses_unbound))
		.service(actix_files::Files::new("/", &dir))
	).bind(format!("0.0.0.0:{}", port)).unwrap().run().await.unwrap();
}

