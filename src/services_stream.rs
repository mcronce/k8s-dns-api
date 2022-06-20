use std::pin::Pin;
use std::vec::IntoIter;

use futures::stream::Stream;
use futures::task::Context;
use futures::task::Poll;
use futures::Future;

use compact_str::CompactString;
use k8s_openapi::api::core::v1::Service;
use kube::api::ListParams;
use kube::core::ObjectList;
use kube::error::Error;
use kube::Api;

enum State {
	Default(IntoIter<Service>),
	LoadingAll(Pin<Box<dyn Future<Output = Result<ObjectList<Service>, Error>>>>),
	All(IntoIter<Service>),
	Done
}

pub struct ServiceStream {
	client: kube::Client,
	tld: CompactString,
	state: State
}

async fn load_all_services(client: kube::Client) -> Result<ObjectList<Service>, Error> {
	Api::<Service>::all(client).list(&ListParams::default()).await
}

impl ServiceStream {
	pub async fn new(client: &kube::Client, tld: &str) -> Result<Self, Error> {
		let services = Api::<Service>::default_namespaced(client.clone()).list(&ListParams::default()).await?;
		Ok(Self{
			client: client.clone(),
			tld: tld.into(),
			state: State::Default(services.into_iter())
		})
	}
}

impl Stream for ServiceStream {
	type Item = Result<(CompactString, String), kube::error::Error>;
	fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		match &mut self.state {
			State::Default(iter) => {
				for service in iter.by_ref() {
					let name = match service.metadata.name.as_ref() {
						None => continue,
						Some(n) => n
					};
					let cluster_ip = match service.spec.and_then(|s| s.cluster_ip) {
						None => continue,
						Some(s) if s == "None" => continue,
						Some(s) => s
					};
					return Poll::Ready(Some(Ok((cluster_ip.into(), format!("{}{}", name, &self.tld)))));
				}
				self.state = State::LoadingAll(Box::pin(load_all_services(self.client.clone())));
				ctx.waker().wake_by_ref();
				Poll::Pending
			},
			State::LoadingAll(fut) => match Pin::new(fut).poll(ctx) {
				Poll::Pending => Poll::Pending,
				Poll::Ready(Ok(services)) => {
					self.state = State::All(services.into_iter());
					ctx.waker().wake_by_ref();
					Poll::Pending
				},
				Poll::Ready(Err(e)) => {
					self.state = State::Done;
					Poll::Ready(Some(Err(e)))
				}
			},
			State::All(iter) => {
				for service in iter.by_ref() {
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
					return Poll::Ready(Some(Ok((cluster_ip.into(), format!("{}.{}{}", name, namespace, &self.tld)))));
				}
				self.state = State::Done;
				Poll::Ready(None)
			}
			State::Done => Poll::Ready(None)
		}
	}
}

