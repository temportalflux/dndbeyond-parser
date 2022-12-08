use anyhow::Context;
use async_channel::{Receiver, Sender};
use futures::Future;
use std::{
	sync::{Arc, Mutex},
	task::{Poll, Waker},
};
use tokio::task::JoinHandle;

pub mod creature_list;

pub struct WebpageProvider {
	client: Arc<reqwest::Client>,
	send_request: Sender<PendingRequest>,
	recv_request: Receiver<PendingRequest>,
}
impl WebpageProvider {
	pub async fn new() -> anyhow::Result<Self> {
		let client = Arc::new(Self::build_client().await?);
		let (send_request, recv_request) = async_channel::unbounded();
		Ok(Self {
			client,
			send_request,
			recv_request,
		})
	}

	async fn build_client() -> anyhow::Result<reqwest::Client> {
		let dndbeyond_url = "https://www.dndbeyond.com".parse::<reqwest::Url>()?;
		let cookie_jar = Self::build_cookie_jar(&dndbeyond_url).await?;
		Ok(reqwest::Client::builder()
			.cookie_provider(cookie_jar.clone())
			.build()?)
	}

	async fn build_cookie_jar(domain: &reqwest::Url) -> anyhow::Result<Arc<reqwest::cookie::Jar>> {
		let cookie_jar = Arc::new(reqwest::cookie::Jar::default());
		let cookies = Self::read_cookies().await?;
		for cookie in cookies.into_iter() {
			cookie_jar.add_cookie_str(&cookie, domain);
		}
		Ok(cookie_jar)
	}

	async fn read_cookies() -> anyhow::Result<Vec<String>> {
		let content = tokio::fs::read_to_string("cookies.txt")
			.await
			.context("Missing 'cookies.txt' file")?;
		let content = content.replace("; ", "\n");
		let entries = content.split("\n");
		Ok(entries.map(|s| s.to_owned()).collect::<Vec<_>>())
	}

	pub fn spawn_workers(&self, count: usize) -> Vec<JoinHandle<()>> {
		static NAMES: [&'static str; 16] = [
			"alpha", "bravo", "canon", "delta", "ephor", "flump", "gnome", "hedge", "igloo",
			"julep", "knoll", "liege", "magic", "novel", "omega", "panda",
		];
		let mut pool_handles = Vec::new();
		for idx in 0..count {
			let client = self.client.clone();
			let channel = self.recv_request.clone();
			let worker_name = NAMES
				.get(idx)
				.map(|s| (*s).to_owned())
				.unwrap_or(format!("worker-{idx}"));
			pool_handles.push(tokio::task::spawn(async move {
				while let Ok(request) = channel.recv().await {
					log::info!(
						target: &worker_name,
						"Fetching {:?}",
						request.url().as_str()
					);
					let result = client.get(request.url().clone()).send().await;
					request.set_response(result.map_err(|_| FetchFailed(request.url().clone())));
					request.wake();
				}
			}));
		}
		pool_handles
	}

	pub fn fetch<TUrl>(&self, url: TUrl) -> anyhow::Result<Request>
	where
		TUrl: reqwest::IntoUrl,
	{
		Request::new(self.send_request.clone(), url)
	}
}

struct PendingRequest(
	reqwest::Url,
	Waker,
	Arc<Mutex<Option<Result<reqwest::Response, FetchFailed>>>>,
);
impl PendingRequest {
	fn url(&self) -> &reqwest::Url {
		&self.0
	}

	fn wake(&self) {
		self.1.wake_by_ref();
	}

	fn set_response(&self, result: Result<reqwest::Response, FetchFailed>) {
		let mut response = self.2.lock().unwrap();
		*response = Some(result);
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub struct FetchFailed(reqwest::Url);
impl std::fmt::Display for FetchFailed {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Failed to fetch url {:?}", self.0)
	}
}

pub struct Request {
	url: reqwest::Url,
	channel: Sender<PendingRequest>,
	response: Option<Arc<Mutex<Option<Result<reqwest::Response, FetchFailed>>>>>,
}
impl Request {
	fn new<TUrl>(channel: Sender<PendingRequest>, url: TUrl) -> anyhow::Result<Self>
	where
		TUrl: reqwest::IntoUrl,
	{
		Ok(Self {
			url: url.into_url()?,
			channel,
			response: None,
		})
	}
}
impl Future for Request {
	type Output = Result<reqwest::Response, FetchFailed>;

	fn poll(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> Poll<Self::Output> {
		// Consume whatever exists in response b/c we cannot clone a true `reqwest::Response`.
		match self.response.take() {
			// The request has not started if there is no response-arctex.
			// We should send a request now.
			None => {
				// Create the arctex which holds the result of the request.
				let response = Arc::new(Mutex::new(None));
				// Save a handle to the arctex in this pending future.
				self.response = Some(response.clone());
				// Send the request to the workers for fetching.
				let _ = self.channel.try_send(PendingRequest(
					// the url to request
					self.url.clone(),
					// this sends a handle so the worker that fetches can awake the future when it is complete.
					cx.waker().clone(),
					// the output of the request
					response,
				));
				// Tell executor that we are waiting to be awoken
				Poll::Pending
			}
			// There is a pending request. We need to check if it has been fulfilled.
			Some(response) => {
				// Take the value from the request-output. The arctex will be empty.
				// Needed because we cannot clone a successful `reqwest::Response`.
				let resp_result = response.lock().unwrap().take();
				// If the request has been fulfilled, tell the executor that the future has finished.
				if let Some(result) = resp_result {
					Poll::Ready(result)
				}
				// The request is still pending, so put the empty arctex back in the future to be queried later.
				else {
					self.response = Some(response);
					Poll::Pending
				}
			}
		}
	}
}
