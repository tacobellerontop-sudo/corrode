//! Credential-free release updates. Work and filesystem access stay off the UI thread.
use eframe::egui;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
	path::PathBuf,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicU64, Ordering},
		mpsc,
	},
	time::{Duration, Instant},
};
use tokio::runtime::Runtime;

#[path = "updater_install.rs"]
mod install;

// Remote update checks were removed with the upstream release feed: this fork
// ships no update endpoint, so every check reports as unconfigured. The local
// download/verify/install machinery below stays wired for a future endpoint.
const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Deserialize)]
struct Asset {
	name: String,
	browser_download_url: String,
	size: u64,
}
#[derive(Clone)]
struct Package {
	version: String,
	// `None` on an installation with no in-app installer: the version is still reported,
	// but there is nothing here to download.
	archive: Option<Asset>,
	checksums: Option<Asset>,
}
struct Staged {
	directory: PathBuf,
	installation: PathBuf,
}
enum Outcome {
	Checked(Option<Package>),
	Downloaded(Staged),
	Prepared(install::Prepared),
	Cleaned,
}
struct Job {
	receiver: mpsc::Receiver<Result<Outcome, String>>,
	cancel: Arc<AtomicBool>,
	progress: Arc<AtomicU64>,
	total: u64,
}
pub struct Updater {
	demo: bool,
	channel: Option<bool>,
	next_check: Instant,
	last_check: Option<Instant>,
	job: Option<Job>,
	package: Option<Package>,
	staged: Option<Staged>,
	cleanup_pending: Option<Staged>,
	status: String,
	armed: bool,
	helper: Option<install::Prepared>,
	close_requested: bool,
	auto_download: bool,
	demo_available: bool,
	demo_ready: bool,
}
impl Updater {
	pub fn new(demo: bool) -> Self {
		Self {
			demo,
			channel: None,
			next_check: Instant::now(),
			last_check: None,
			job: None,
			package: None,
			staged: None,
			cleanup_pending: None,
			status: "Updates have not been checked yet.".into(),
			armed: false,
			helper: None,
			close_requested: false,
			auto_download: false,
			demo_available: false,
			demo_ready: false,
		}
	}

	/// Returns true only when a prepared restart should enter the normal close/draft gate.
	pub fn sync(
		&mut self,
		ctx: &egui::Context,
		runtime: &Runtime,
		view: &mut ui::updates::Updates,
		enabled: bool,
	) -> bool {
		let check = std::mem::take(&mut view.check_requested);
		let download = std::mem::take(&mut view.download_requested);
		let restart = std::mem::take(&mut view.restart_requested);
		if self.demo {
			if self.channel != Some(view.nightly) {
				self.demo_available = false;
				self.demo_ready = false;
			}
			self.channel = Some(view.nightly);
			if check {
				self.demo_available = true;
				self.status =
					"Synthetic preview: Corrode 99.0.0 is available. No network request was made."
						.into();
			}
			if download && self.demo_available {
				self.demo_ready = true;
				self.status =
					"Synthetic preview: update ready to restart. No files were downloaded.".into();
			}
			if restart && self.demo_ready {
				self.status =
					"Synthetic preview: restart simulated. No installation was changed.".into();
			}
			view.status.clone_from(&self.status);
			view.available = self.demo_available;
			view.ready = self.demo_ready;
			view.busy = false;
			view.progress = None;
			view.supported = true;
			return false;
		}
		view.supported = true;
		if !enabled {
			if let Some(job) = &self.job {
				job.cancel.store(true, Ordering::Relaxed);
			}
			view.busy = self.job.is_some();
			view.available = false;
			view.ready = false;
			view.progress = None;
			view.status = if cfg!(debug_assertions) {
				"Update checks are disabled in debug builds."
			} else {
				"Load update preferences or choose your update settings to enable checking."
			}
			.into();
			return false;
		}
		if self.channel != Some(view.nightly) {
			if let Some(job) = &self.job {
				job.cancel.store(true, Ordering::Relaxed);
			}
			self.channel = Some(view.nightly);
			self.package = None;
			self.auto_download = false;
			self.cancel_restart();
			self.discard_stage();
			self.next_check = Instant::now();
			self.last_check = None;
			self.status = "Updates have not been checked on this channel yet.".into();
		}
		if let Some(job) = &self.job {
			match job.receiver.try_recv() {
				Ok(result) => {
					let cancelled = job.cancel.load(Ordering::Relaxed);
					self.job = None;
					if cancelled {
						match result {
							Ok(Outcome::Downloaded(stage)) => {
								self.cleanup_pending = Some(stage);
							}
							Ok(Outcome::Prepared(helper)) => {
								self.helper = Some(helper);
							}
							_ => {}
						}
					} else {
						match result {
							Ok(Outcome::Checked(package)) => {
								self.status = package.as_ref().map_or_else(
									|| "Corrode is up to date on this channel.".into(),
									|p| format!("Corrode {} is available.", p.version),
								);
								self.package = package;
								self.auto_download = true;
							}
							Ok(Outcome::Downloaded(stage)) => {
								self.staged = Some(stage);
								self.status =
									"Update downloaded and verified. Restart when you are ready."
										.into();
							}
							Ok(Outcome::Cleaned) => {}
							Ok(Outcome::Prepared(helper)) => {
								self.helper = Some(helper);
								self.armed = true;
								self.close_requested = true;
								self.status =
									"Update ready. Close Corrode to install and restart.".into();
							}
							Err(error) => {
								self.auto_download = false;
								self.status = error;
								self.next_check = Instant::now() + CHECK_INTERVAL;
							}
						}
					}
				}
				Err(mpsc::TryRecvError::Disconnected) => {
					self.job = None;
					self.status = "The update worker stopped. Please try again.".into();
				}
				Err(mpsc::TryRecvError::Empty) => {}
			}
		}
		if self.job.is_none() {
			if let Some(stage) = self.cleanup_pending.take() {
				let helper = self.helper.take();
				self.start(runtime, ctx, 0, move |_, _| async move {
					tokio::task::spawn_blocking(move || {
						if let Some(helper) = helper {
							helper.stop();
						}
						install::cleanup(&stage.directory);
					})
					.await
					.map_err(|_| "Could not clear old update storage.".to_owned())?;
					Ok(Outcome::Cleaned)
				});
			} else if restart && self.staged.is_some() && !self.armed {
				let stage = self.staged.as_ref().expect("checked stage");
				let directory = stage.directory.clone();
				let installation = stage.installation.clone();
				let version = self.package.as_ref().map(|p| p.version.clone());
				let old_helper = self.helper.take();
				self.start(runtime, ctx, 0, move |cancel, _| async move {
					let helper = tokio::task::spawn_blocking(move || {
						if let Some(helper) = old_helper {
							helper.stop();
						}
						if cancel.load(Ordering::Relaxed) {
							return Err("Update cancelled.".into());
						}
						install::prepare_restart(&directory, &installation, version.as_deref())
					})
					.await
					.map_err(|_| "Could not prepare the update restart.".to_owned())??;
					Ok(Outcome::Prepared(helper))
				});
				self.status = "Preparing to restart…".into();
			} else if (download || (view.auto_update && self.auto_download))
				&& self.package.is_some()
				&& self.staged.is_none()
			{
				let package = self.package.as_ref().expect("checked package").clone();
				let total = package
					.archive
					.as_ref()
					.expect("a supported platform's checked package has a downloadable archive")
					.size;
				self.auto_download = false;
				self.status = format!("Downloading Corrode {}…", package.version);
				self.start(runtime, ctx, total, move |cancel, progress| {
					download_package(package, cancel, progress)
				});
			} else if self.staged.is_none() && (check || Instant::now() >= self.next_check) {
				if check
					&& self
						.last_check
						.is_some_and(|last| last.elapsed() < Duration::from_secs(60))
				{
					self.status = "Please wait one minute between update checks.".into();
				} else {
					self.last_check = Some(Instant::now());
					self.next_check = Instant::now() + CHECK_INTERVAL;
					let nightly = view.nightly;
					self.start(runtime, ctx, 0, move |cancel, _| async move {
						check_release(nightly, cancel).await.map(Outcome::Checked)
					});
					self.status = "Checking for updates…".into();
				}
			}
		}
		view.busy = self.job.is_some();
		view.progress = self.job.as_ref().filter(|job| job.total > 0).map(|job| {
			(job.progress.load(Ordering::Relaxed) as f64 / job.total as f64).min(1.0) as f32
		});
		view.available = self.package.is_some()
			|| self.staged.is_some()
			|| self.job.as_ref().is_some_and(|job| job.total > 0);
		view.ready = self.staged.is_some() && self.job.is_none();
		view.status.clone_from(&self.status);
		if self.job.is_some() {
			ctx.request_repaint_after(Duration::from_millis(200));
		} else if self.staged.is_none() {
			ctx.request_repaint_after(
				self.next_check
					.saturating_duration_since(Instant::now())
					.max(Duration::from_secs(1)),
			);
		}
		std::mem::take(&mut self.close_requested)
	}
	fn start<F, Fut>(&mut self, runtime: &Runtime, ctx: &egui::Context, total: u64, work: F)
	where
		F: FnOnce(Arc<AtomicBool>, Arc<AtomicU64>) -> Fut + Send + 'static,
		Fut: std::future::Future<Output = Result<Outcome, String>> + Send + 'static,
	{
		let (sender, receiver) = mpsc::sync_channel(1);
		let cancel = Arc::new(AtomicBool::new(false));
		let progress = Arc::new(AtomicU64::new(0));
		let worker_cancel = Arc::clone(&cancel);
		let worker_progress = Arc::clone(&progress);
		let ctx = ctx.clone();
		runtime.spawn(async move {
			let result = work(worker_cancel, worker_progress)
				.await
				.map_err(|error| format!("Update failed: {error}"));
			if let Err(mpsc::SendError(result)) = sender.send(result) {
				tokio::task::spawn_blocking(move || match result {
					Ok(Outcome::Downloaded(stage)) => install::cleanup(&stage.directory),
					Ok(Outcome::Prepared(helper)) => helper.stop(),
					_ => {}
				});
			}
			ctx.request_repaint();
		});
		self.job = Some(Job {
			receiver,
			cancel,
			progress,
			total,
		});
	}
	fn discard_stage(&mut self) {
		if let Some(stage) = self.staged.take() {
			self.cleanup_pending = Some(stage);
		}
	}

	pub fn cancel_restart(&mut self) {
		self.armed = false;
		self.close_requested = false;
		if let Some(job) = &self.job {
			job.cancel.store(true, Ordering::Relaxed);
		}
	}

	/// Called only during the application's approved shutdown, after draft/close gates.
	pub fn finish_restart(&mut self) -> Result<(), String> {
		if self.demo || !self.armed {
			return Ok(());
		}
		if self.staged.is_some() {
			std::fs::write(
				&self
					.helper
					.as_ref()
					.ok_or("No update helper is ready.")?
					.marker,
				[],
			)
			.map_err(|_| "Could not hand off the update to the installer.".to_owned())?;
			self.staged = None; // The helper owns cleanup after this handoff.
			self.helper = None;
		}
		self.armed = false;
		Ok(())
	}
}
impl Drop for Updater {
	fn drop(&mut self) {
		if let Some(job) = &self.job {
			job.cancel.store(true, Ordering::Relaxed);
		}
		let helper = self.helper.take();
		let stage = self.staged.take().or_else(|| self.cleanup_pending.take());
		if helper.is_some() || stage.is_some() {
			std::thread::spawn(move || {
				if let Some(helper) = helper {
					helper.stop();
				}
				if let Some(stage) = stage {
					install::cleanup(&stage.directory);
				}
			});
		}
	}
}

fn client() -> Result<reqwest::Client, String> {
	reqwest::Client::builder()
		.https_only(true)
		.no_proxy()
		.user_agent(concat!("Corrode/", env!("CARGO_PKG_VERSION")))
		.connect_timeout(Duration::from_secs(10))
		.read_timeout(Duration::from_secs(30))
		.timeout(Duration::from_secs(600))
		.redirect(reqwest::redirect::Policy::custom(|attempt| {
			if attempt.previous().len() < 5 && trusted_url(attempt.url()) {
				attempt.follow()
			} else {
				attempt.error("Untrusted update redirect")
			}
		}))
		.build()
		.map_err(|_| "Could not initialize secure update transport.".into())
}
fn trusted_url(url: &url::Url) -> bool {
	url.scheme() == "https"
		&& url.port_or_known_default() == Some(443)
		&& url.username().is_empty()
		&& url.password().is_none()
		&& url.fragment().is_none()
		&& url.host_str().is_some()
}
async fn response(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
	if !url::Url::parse(url).is_ok_and(|url| trusted_url(&url)) {
		return Err("The release contains an untrusted download address.".into());
	}
	let response = client
		.get(url)
		.send()
		.await
		.map_err(|_| "Could not reach the update server. Check your connection and try again.".to_owned())?;
	match response.status().as_u16() {
		200 => Ok(response),
		403 | 429 => Err("The update server limit was reached. Try again later.".into()),
		404 => Err("No published release is available on this channel yet.".into()),
		_ => Err("The update server could not provide the update. Try again later.".into()),
	}
}
async fn bounded_body(
	client: &reqwest::Client,
	url: &str,
	limit: usize,
	cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
	let mut response = response(client, url).await?;
	if response
		.content_length()
		.is_some_and(|length| length > limit as u64)
	{
		return Err("Release metadata exceeds the size limit.".into());
	}
	let mut body = Vec::new();
	while let Some(chunk) = response
		.chunk()
		.await
		.map_err(|_| "The update response was interrupted.".to_owned())?
	{
		if cancel.load(Ordering::Relaxed) {
			return Err("Update cancelled.".into());
		}
		if chunk.len() > limit.saturating_sub(body.len()) {
			return Err("Release metadata exceeds the size limit.".into());
		}
		body.extend_from_slice(&chunk);
	}
	Ok(body)
}
async fn check_release(_nightly: bool, _cancel: Arc<AtomicBool>) -> Result<Option<Package>, String> {
	Err("Automatic updates are not configured in this fork.".into())
}
fn checksum(body: &[u8], name: &str) -> Result<[u8; 32], String> {
	let body = std::str::from_utf8(body).map_err(|_| "The checksum file is invalid.".to_owned())?;
	let mut result = None;
	for line in body.lines() {
		let mut fields = line.split_whitespace();
		let (Some(hash), Some(file)) = (fields.next(), fields.next()) else {
			continue;
		};
		if file
			.trim_start_matches('*')
			.strip_prefix("./")
			.unwrap_or(file.trim_start_matches('*'))
			!= name
		{
			continue;
		}
		if result.is_some()
			|| fields.next().is_some()
			|| hash.len() != 64
			|| !hash.bytes().all(|b| b.is_ascii_hexdigit())
		{
			return Err("The package checksum is invalid or duplicated.".into());
		}
		let mut bytes = [0; 32];
		for (i, byte) in bytes.iter_mut().enumerate() {
			*byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)
				.map_err(|_| "Invalid checksum.".to_owned())?;
		}
		result = Some(bytes);
	}
	result.ok_or_else(|| "The release has no checksum for this package.".into())
}
async fn download_package(
	package: Package,
	cancel: Arc<AtomicBool>,
	progress: Arc<AtomicU64>,
) -> Result<Outcome, String> {
	// Only reachable on a platform with an in-app installer, when a checked
	// package actually carries downloadable archives.
	let archive = package
		.archive
		.ok_or("This platform cannot install updates in-app.")?;
	let checksums = package
		.checksums
		.ok_or("This platform cannot install updates in-app.")?;
	let client = client()?;
	let checksum_body =
		bounded_body(&client, &checksums.browser_download_url, 64 * 1024, &cancel).await?;
	let expected = checksum(&checksum_body, &archive.name)?;
	let stage = tokio::task::spawn_blocking(install::create_stage)
		.await
		.map_err(|_| "Could not prepare update storage.".to_owned())??;
	let directory = stage.directory.clone();
	let result = async {
		if cancel.load(Ordering::Relaxed) {
			return Err("Update cancelled.".into());
		}
		let partial = directory.join("package.zip");
		match tokio::fs::remove_file(&partial).await {
			Ok(()) => {}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
			Err(_) => return Err("Could not discard the partial update.".into()),
		}
		progress.store(0, Ordering::Relaxed);
		download_full(&client, &archive, &directory, &expected, &cancel, &progress).await?;
		let path = directory.clone();
		tokio::task::spawn_blocking(move || install::unpack(&path, &cancel))
			.await
			.map_err(|_| "Could not unpack the update.".to_owned())??;
		Ok(())
	}
	.await;
	match result {
		Ok(()) => Ok(Outcome::Downloaded(stage)),
		Err(error) => {
			tokio::task::spawn_blocking(move || install::cleanup(&directory))
				.await
				.ok();
			Err(error)
		}
	}
}

async fn download_full(
	client: &reqwest::Client,
	archive: &Asset,
	directory: &std::path::Path,
	expected: &[u8; 32],
	cancel: &AtomicBool,
	progress: &AtomicU64,
) -> Result<(), String> {
	use tokio::io::AsyncWriteExt;
	let mut response = response(client, &archive.browser_download_url).await?;
	if response
		.content_length()
		.is_some_and(|size| size != archive.size)
	{
		return Err("The package size does not match its release metadata.".into());
	}
	let mut file = tokio::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(directory.join("package.zip"))
		.await
		.map_err(|_| "Could not create the update download.".to_owned())?;
	let mut hasher = Sha256::new();
	let mut received = 0_u64;
	while let Some(chunk) = response
		.chunk()
		.await
		.map_err(|_| "The update download was interrupted.".to_owned())?
	{
		if cancel.load(Ordering::Relaxed) {
			return Err("Update cancelled.".into());
		}
		received = received
			.checked_add(chunk.len() as u64)
			.ok_or("Update size overflow.")?;
		if received > archive.size || received > MAX_DOWNLOAD {
			return Err("The downloaded package exceeds its size limit.".into());
		}
		file.write_all(&chunk)
			.await
			.map_err(|_| "Could not write the update. Check available disk space.".to_owned())?;
		hasher.update(&chunk);
		progress.store(received, Ordering::Relaxed);
	}
	if received != archive.size || hasher.finalize().as_slice() != expected.as_slice() {
		return Err("The update checksum or length did not match. Nothing was installed.".into());
	}
	file.sync_all()
		.await
		.map_err(|_| "Could not save the update package.".to_owned())?;
	drop(file);
	Ok(())
}

/// Offline checks used by the explicit demo debug path, never by release startup.
#[cfg(feature = "demo")]
pub fn debug_check() -> Result<(), String> {
	for address in [
		"http://example.com/x",
		"https://user@example.com/x",
		"https://example.com:444/x",
		"https://example.com/x#fragment",
	] {
		if trusted_url(&url::Url::parse(address).map_err(|_| "URL parse")?) {
			return Err("Untrusted origin accepted.".into());
		}
	}
	if !trusted_url(&url::Url::parse("https://example.com/x").map_err(|_| "URL parse")?) {
		return Err("Trusted origin rejected.".into());
	}
	let hash = "00".repeat(32);
	if checksum(format!("{hash}  ./package.zip\n").as_bytes(), "package.zip")? != [0; 32]
		|| checksum(
			format!("{hash}  ./package.zip\n{hash}  ./package.zip\n").as_bytes(),
			"package.zip",
		)
		.is_ok()
	{
		return Err("Checksum validation failed.".into());
	}
	install::debug_check()
}
