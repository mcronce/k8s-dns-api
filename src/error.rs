use std::convert::TryFrom;

use actix_web::body::BoxBody;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::HttpResponseBuilder;
use actix_web::ResponseError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Ingress not found")]
	IngressNotFound,
	#[error("Kubernetes error:  {0}")]
	Kube(#[from] kube::Error)
}

impl ResponseError for Error {
	fn status_code(&self) -> StatusCode {
		match self {
			Self::IngressNotFound => StatusCode::NOT_FOUND,
			Self::Kube(inner) => match inner {
				kube::Error::Api(response) => match StatusCode::try_from(response.code) {
					Ok(v) => v,
					Err(_) => StatusCode::INTERNAL_SERVER_ERROR
				},
				kube::Error::HyperError(_) => StatusCode::BAD_GATEWAY,
				kube::Error::Service(_) => StatusCode::BAD_GATEWAY,
				kube::Error::FromUtf8(_) => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::LinesCodecMaxLineLengthExceeded => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::ReadEvents(_) => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::HttpError(_) => StatusCode::BAD_GATEWAY,
				kube::Error::SerdeError(_) => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::BuildRequest(_) => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::InferConfig(_) => StatusCode::BAD_GATEWAY,
				kube::Error::Discovery(_) => StatusCode::BAD_GATEWAY,
				//kube::Error::NativeTls(_) => StatusCode::INTERNAL_SERVER_ERROR,
				kube::Error::OpensslTls(_) => StatusCode::INTERNAL_SERVER_ERROR,
				//kube::Error::RustlsTls(_) => StatusCode::INTERNAL_SERVER_ERROR,
				//kube::Error::UpgradeConnection(_) => StatusCode::BAD_GATEWAY,
				kube::Error::Auth(_) => StatusCode::UNAUTHORIZED
			}
		}
	}

	fn error_response(&self) -> HttpResponse<BoxBody> {
		HttpResponseBuilder::new(self.status_code()).body(self.to_string())
	}
}

