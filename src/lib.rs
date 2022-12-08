use futures::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

// Alias used to represent a future that can be returned from a trait function,
// because async is not supported for trait functions yet.
pub type PinFuture<T> = PinFutureLifetime<'static, T>;
pub type PinFutureLifetime<'l, T> = Pin<Box<dyn Future<Output = T> + 'l + Send>>;

#[profiling::function]
pub fn create_runtime() -> tokio::runtime::Runtime {
	let thread_count = num_cpus::get();
	let mut builder = tokio::runtime::Builder::new_multi_thread();
	builder.enable_all();
	builder.worker_threads(thread_count);
	builder.thread_name_fn(|| {
		use std::sync::atomic::{AtomicUsize, Ordering};
		static THREAD_ID: AtomicUsize = AtomicUsize::new(0);
		let id = THREAD_ID.fetch_add(1, Ordering::SeqCst);
		create_task_thread_name(id)
	});
	let async_runtime = builder.build().unwrap();
	// Thread Registration
	{
		profiling::scope!("spawn-registration-tasks");
		let arclock = Arc::new(RwLock::new(0));
		for _ in 0..thread_count {
			let thread_counter = arclock.clone();
			async_runtime.spawn(async move {
				register_worker_thread(thread_counter, thread_count);
			});
		}
	}
	async_runtime
}

fn create_task_thread_name(idx: usize) -> String {
	static NAMES: [&'static str; 16] = [
		"alpha", "bravo", "canon", "delta", "ephor", "flump", "gnome", "hedge", "igloo", "julep",
		"knoll", "liege", "magic", "novel", "omega", "panda",
	];
	if idx < NAMES.len() {
		NAMES[idx].to_owned()
	} else {
		format!("task-worker:{}", idx)
	}
}

fn register_worker_thread(thread_counter: Arc<RwLock<usize>>, thread_count: usize) {
	profiling::register_thread!();
	profiling::scope!("register_worker_thread");
	static THREAD_DELAY: std::time::Duration = std::time::Duration::from_millis(1);
	if let Ok(mut counter) = thread_counter.write() {
		*counter += 1;
	}
	// Block the worker thread until all threads have been registered.
	while *thread_counter.read().unwrap() < thread_count {
		std::thread::sleep(THREAD_DELAY);
	}
}

pub fn spawn<T, R>(target: String, future: T) -> tokio::task::JoinHandle<Option<R>>
where
	T: futures::future::Future<Output = anyhow::Result<R>> + Send + 'static,
	R: Send + 'static,
{
	tokio::task::spawn(async move {
		match future.await {
			Ok(ret) => Some(ret),
			Err(err) => {
				log::error!(target: &target, "{:?}", err);
				None
			}
		}
	})
}

pub async fn join_all<I, S>(iter: I) -> anyhow::Result<(Vec<S>, Vec<anyhow::Error>)>
where
	I: IntoIterator,
	I::Item: Future<Output = Result<anyhow::Result<S>, tokio::task::JoinError>> + Send,
{
	let mut errors = Vec::new();
	let mut items = Vec::new();
	let results = futures::future::join_all(iter).await;
	for result in results.into_iter() {
		match result {
			Ok(Ok(item)) => {
				items.push(item);
			}
			// if any are errors, then return that one of the internal items failed
			Ok(Err(e)) => {
				errors.push(e);
			}
			Err(_) => {}
		}
	}
	Ok((items, errors))
}
