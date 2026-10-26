use crate::{
	Error, Result,
	core::{
		handlers::certs::CertBackgroundRenewal,
		models::{
			certs::{TlsMaterial, TlsStore},
			proxy::{ProxyConfig, ProxyRoute, ProxyRouteMap},
		},
	},
	error,
};
use async_trait::async_trait;
use http::header;
use pingora::{
	ErrorType,
	http::{RequestHeader, ResponseHeader},
	listeners::{TlsAccept, tls::TlsSettings},
	prelude::{Error as PingoraError, HttpPeer, Result as PingoraResult, background_service},
	proxy::{ProxyHttp, Session, http_proxy_service},
	server::{Server, configuration::ServerConf},
	services::Service,
	tls::{self, ssl},
};
use std::{collections::HashMap, net::IpAddr, sync::Arc};

pub fn run_proxy(
	proxy_config: ProxyConfig,
	routes: Vec<ProxyRoute>,
	tls_store: TlsStore,
	renewal: CertBackgroundRenewal,
) -> Result<()> {
	let mut server = Server::new(None).map_err(|e| Error::Proxy(e.to_string()))?;

	server.bootstrap();

	let server_conf = server.configuration.clone();
	let http_addr = format!("{}:{}", proxy_config.input_address, *proxy_config.http_port);
	let https_addr = format!("{}:{}", proxy_config.input_address, *proxy_config.https_port);
	let mut routes_map = HashMap::new();

	for route in routes {
		routes_map.insert(route.host.clone(), route);
	}

	let routes_map = Arc::new(routes_map);

	// plain proxies with redirect
	let plain_service =
		plain_routes_service(server_conf.clone(), http_addr.clone(), routes_map.clone())?;
	server.add_service(plain_service);

	// tls proxies
	let tls_service = tls_routes_service(server_conf, https_addr, routes_map, tls_store)?;
	server.add_service(tls_service);

	// background cert services
	server.add_service(background_service("cert-renewal", renewal));

	server.run_forever();
}

pub fn tls_routes_service(
	server_conf: Arc<ServerConf>,
	listen_addr: String,
	routes_map: ProxyRouteMap,
	tls_store: TlsStore,
) -> Result<impl Service> {
	let proxy_app = ProxyToUpstream::new(routes_map.clone(), false);

	let mut service = http_proxy_service(&server_conf, proxy_app);

	let cert_resolver = CertResolver::new(tls_store);
	let callback = Box::new(cert_resolver);
	let mut tls_settings =
		TlsSettings::with_callbacks(callback).map_err(|e| Error::Proxy(e.to_string()))?;
	tls_settings.enable_h2();
	service.add_tls_with_settings(&listen_addr, None, tls_settings);

	Ok(service)
}

pub fn plain_routes_service(
	server_conf: Arc<ServerConf>,
	listen_addr: String,
	routes_map: ProxyRouteMap,
) -> Result<impl Service> {
	let proxy_app = ProxyToUpstream::new(routes_map, true);

	let mut service = http_proxy_service(&server_conf, proxy_app);
	service.add_tcp(&listen_addr);

	Ok(service)
}

pub struct ProxyToUpstream {
	routes_map: ProxyRouteMap,
	upgrade_to_https: bool,
}

impl ProxyToUpstream {
	pub fn new(routes_map: ProxyRouteMap, upgrade_to_https: bool) -> Self {
		Self {
			routes_map,
			upgrade_to_https,
		}
	}
}

#[async_trait]
impl ProxyHttp for ProxyToUpstream {
	type CTX = ();
	fn new_ctx(&self) {}

	// ProxyToUpstream is shared with both HTTP and HTTPS
	// here we do a quick check to upgrade to HTTPS if its a TLS route
	async fn request_filter(&self, session: &mut Session, _ctx: &mut ()) -> PingoraResult<bool> {
		if self.upgrade_to_https {
			let host = host_from_session(session)
				// 400 bad request no host
				.ok_or_else(|| PingoraError::new(ErrorType::HTTPStatus(400)))?;
			match self.routes_map.get(host) {
				// 421 Misdirected Request doesnt match any hosts
				None => {
					error!("no route for host: {}", host);
					let header = ResponseHeader::build(421, None)?;
					session.write_response_header(Box::new(header), true).await?;
					return Ok(true);
				},
				// 301 Moved Permanently redirect to https
				Some(route) if *route.tls => {
					let mut header = ResponseHeader::build(301, None)?;
					header.insert_header(header::LOCATION, format!("https://{}", route.host))?;
					header.insert_header(header::CONTENT_LENGTH, 0)?;

					session.write_response_header(Box::new(header), true).await?;

					return Ok(true);
				},
				// All good let it pass.
				Some(_) => return Ok(false),
			}
		}

		Ok(false)
	}

	async fn upstream_peer(
		&self,
		session: &mut Session,
		_ctx: &mut (),
	) -> PingoraResult<Box<HttpPeer>> {
		let host = host_from_session(session)
			// 400 bad request no host
			.ok_or_else(|| PingoraError::new(ErrorType::HTTPStatus(400)))?;

		let route = self
			.routes_map
			.get(host)
			// 421 Misdirected Request doesnt match any hosts
			.ok_or_else(|| PingoraError::new(ErrorType::HTTPStatus(421)))?;

		let proxy_to = HttpPeer::new(route.upstream.as_str(), false, route.host.to_string());
		let peer = Box::new(proxy_to);
		Ok(peer)
	}

	async fn upstream_request_filter(
		&self,
		session: &mut Session,
		upstream_request: &mut RequestHeader,
		_ctx: &mut Self::CTX,
	) -> PingoraResult<()> {
		let ip = ip_from_session(session);

		if let Some(ip) = ip {
			upstream_request.insert_header("X-Forwarded-For", ip.to_string())?
		};

		upstream_request.insert_header(
			"X-Forwarded-Proto",
			if self.upgrade_to_https {
				"http"
			} else {
				"https"
			},
		)?;

		Ok(())
	}
}

// h2 has no Host header, the host comes in :authority instead
// the port is stripped so Host: example.com:8443 matches example.com
fn host_from_session(session: &Session) -> Option<&str> {
	let authority = session
		.get_header(header::HOST)
		.and_then(|h| h.to_str().ok())
		.or_else(|| session.req_header().uri.authority().map(|a| a.as_str()))?;

	match authority.rsplit_once(':') {
		// a ':' inside [] belongs to an IPv6 literal, not a port
		Some((host, _)) if !authority.ends_with(']') => Some(host),
		_ => Some(authority),
	}
}

fn ip_from_session(session: &Session) -> Option<IpAddr> {
	session.client_addr()?.as_inet().map(|addr| addr.ip())
}

struct CertResolver {
	tls_store: TlsStore,
}

impl CertResolver {
	fn new(tls_store: TlsStore) -> Self {
		Self {
			tls_store,
		}
	}
}

#[async_trait]
impl TlsAccept for CertResolver {
	async fn certificate_callback(&self, ssl: &mut ssl::SslRef) -> () {
		let sni_provided = ssl.servername(ssl::NameType::HOST_NAME).map(str::to_owned);

		let Some(sni_provided) = sni_provided else {
			error!("No SNI provided");
			return;
		};

		let certs = self.tls_store.load();

		let Some(TlsMaterial {
			cert,
			chain,
			key,
		}) = certs.get(sni_provided.as_str())
		else {
			error!("No certificate found for SNI: {}", sni_provided);
			return;
		};

		if let Err(e) = tls::ext::ssl_use_certificate(ssl, cert) {
			error!("Failed to use certificate for SNI {}: {}", sni_provided, e);
			return;
		}

		for intermediate in chain {
			if let Err(e) = tls::ext::ssl_add_chain_cert(ssl, intermediate) {
				error!("Failed to add chain certificate for SNI {}: {}", sni_provided, e);
				return;
			}
		}

		if let Err(e) = tls::ext::ssl_use_private_key(ssl, key) {
			error!("Failed to use private key for SNI {}: {}", sni_provided, e);
			return;
		}
	}
}
