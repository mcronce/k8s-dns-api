use std::vec::IntoIter;

use futures::Stream;

use compact_str::CompactString;
use k8s_openapi::api::core::v1::Service;
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::api::networking::v1::IngressRule;
use kube::api::ListParams;
use kube::Api;

use crate::error::Error;

fn format_name(name: &str, tld: &str) -> CompactString {
	let mut name: CompactString = name.into();
	name.push_str(tld);
	name
}

pub struct IngressStream {
	tld: CompactString,
	service_ip: CompactString,
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
	pub async fn new(client: &kube::Client, tld: &str) -> Result<impl Stream<Item = Result<(CompactString, CompactString), !>>, Error> {
		let ingresses = Api::<Ingress>::all(client.clone()).list(&ListParams::default()).await?;
		// TODO:  Allow configurable name
		let service_ip = find_ingress_controller_service(client, "ingress-nginx-internal-controller").await?;
		Ok(futures::stream::iter(Self{
			tld: tld.into(),
			service_ip: service_ip.into(),
			ingresses: ingresses.into_iter(),
			current_ingress: None
		}))
	}
}

impl Iterator for IngressStream {
	type Item = Result<(CompactString, CompactString), !>;
	fn next(&mut self) -> Option<Self::Item> {
		if let Some(rules) = &mut self.current_ingress {
			for rule in rules {
				if let Some(host) = &rule.host {
					return Some(Ok((self.service_ip.clone(), format_name(host, &self.tld))));
				}
			}
			self.current_ingress = None;
		}

		while let Some(ingress) = self.ingresses.next() {
			let spec = match ingress.spec {
				None => continue,
				Some(spec) => spec
			};
			let class = match spec.ingress_class_name {
				None => continue,
				Some(class) => class
			};
			if(class != "internal") {
				continue;
			}
			let rules = match spec.rules {
				None => continue,
				Some(rules) => rules
			};
			self.current_ingress = Some(rules.into_iter());
			return self.next();
		}
		None
	}
}

