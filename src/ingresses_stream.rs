use std::vec::IntoIter;

use futures::Stream;

use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::networking::v1::IngressRule;
use kube::api::ListParams;
use kube::Api;

use crate::error::Error;

pub struct IngressStream {
	tld: String,
	service_ip: String,
	ingresses: IntoIter<Ingress>,
	current_ingress: Option<IntoIter<IngressRule>>
}

async fn find_ingress_controller_service(client: &kube::Client, service_name: &str) -> Result<String, Error> {
	let list = Api::<Service>::namespaced(client.clone(), "kube-system").list(&ListParams::default()).await?;
	for service in list.into_iter() {
		let name = match service.metadata.name.as_ref() {
			Some(v) => v,
			None => continue
		};
		if(name != service_name) {
			continue;
		}
		match &service.spec.and_then(|s| s.cluster_ip) {
			None => return Err(Error::IngressNotFound),
			Some(s) if s == "None" => return Err(Error::IngressNotFound),
			Some(s) => return Ok(s.to_owned())
		};
	}
	Err(Error::IngressNotFound)
}

impl IngressStream {
	#![allow(clippy::new_ret_no_self)] // IngressStream will likely impl Stream in the future, and this will reduce changes at the call sites
	pub async fn new(client: &kube::Client, tld: &str) -> Result<impl Stream<Item = Result<(String, String), !>>, Error> {
		let ingresses = Api::<Ingress>::all(client.clone()).list(&ListParams::default()).await?;
		// TODO:  Allow configurable name
		let service_ip = find_ingress_controller_service(client, "ingress-nginx-internal-controller").await?;
		Ok(futures::stream::iter(Self{
			tld: tld.to_owned(),
			service_ip,
			ingresses: ingresses.into_iter(),
			current_ingress: None
		}))
	}
}

impl Iterator for IngressStream {
	type Item = Result<(String, String), !>;
	fn next(&mut self) -> Option<Self::Item> {
		if let Some(rules) = &mut self.current_ingress {
			for rule in rules {
				if let Some(host) = &rule.host {
					return Some(Ok((self.service_ip.clone(), format!("{}{}", host, &self.tld))));
				}
			}
			self.current_ingress = None;
		}

		while let Some(ingress) = self.ingresses.next() {
			let class = match ingress.spec.as_ref().and_then(|s| s.ingress_class_name.as_ref()) {
				None => continue,
				Some(class) => class
			};
			if(class != "internal") {
				continue;
			}
			let rules = match ingress.spec.and_then(|s| s.rules) {
				None => continue,
				Some(rules) => rules
			};
			self.current_ingress = Some(rules.into_iter());
			return self.next();
		}
		None
	}
}

