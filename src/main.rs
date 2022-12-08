use std::sync::Arc;

use creature::Creature;
use dndbeyond::{creature_list::CreatureListing, WebpageProvider};

pub mod creature;
pub mod dndbeyond;
pub mod utility;

fn main() -> anyhow::Result<()> {
	log_base::init(
		std::env!("CARGO_PKG_NAME"),
		&[
			"html5ever",
			"mio",
			"want",
			"selectors",
			"reqwest",
			"cookie_store",
			"hyper",
			"tracing",
		],
	)?;
	let runtime = async_runtime::create_runtime();
	runtime.block_on(async { run().await })
}

async fn run() -> anyhow::Result<()> {
	let worker_tasks;
	let creatures;
	{
		let provider = Arc::new(WebpageProvider::new().await?);
		// The number of worker tasks spawned here is the number of
		// webpage fetch/get requests that can be processed in parallel.
		worker_tasks = provider.spawn_workers(10);

		let (send_creature, recv_creature) = async_channel::unbounded();

		CreatureListing::fetch_all(provider.clone(), send_creature, Some(0..8)).await?;
		let creature_tasks = fetch_creature_pages(provider, recv_creature);

		creatures = creature_tasks.await??;
	}

	// Technically, if all the work has finished, then these tasks could be dropped without caring
	// if the channels still exist (because they are garunteed to be empty).
	// For the sake of consistency, we stitch the worker tasks back into main thread.
	// If this hangs, its because the sender channel for the requests still exists (it lives in the WebpageProvider).
	futures::future::join_all(worker_tasks).await;

	log::debug!("Finished collecting {} creatures", creatures.len());
	//log::debug!("{creatures:?}");

	Ok(())
}

fn fetch_creature_pages(
	provider: Arc<WebpageProvider>,
	channel: async_channel::Receiver<CreatureListing>,
) -> tokio::task::JoinHandle<anyhow::Result<Vec<Creature>>> {
	let creature_collector = tokio::task::spawn(async move {
		let mut parsing_tasks = Vec::new();
		while let Ok(listing) = channel.recv().await {
			let provider = provider.clone();

			match listing.name().as_str() {
				"Almiraj" | "Avatar of Death" | "Chwinga Astronaut" | "Guard" | "Frog" => {}
				_ => continue,
			}

			parsing_tasks.push(tokio::task::spawn(async move {
				match listing.fetch_full(&provider).await {
					Ok(creature) => Some(creature),
					Err(err) => {
						log::error!("{err:?}");
						None
					}
				}
			}));
		}
		parsing_tasks
	});
	tokio::task::spawn(async move {
		let creature_tasks = creature_collector.await?;
		let creatures = futures::future::join_all(creature_tasks).await;
		let creatures = creatures
			.into_iter()
			.filter_map(|res| res.ok().flatten())
			.collect::<Vec<_>>();
		Ok(creatures) as anyhow::Result<Vec<Creature>>
	})
}
