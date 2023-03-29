use super::WebpageProvider;
use crate::creature::Creature;
use anyhow::Context;
use std::{ops::Range, path::PathBuf, str::FromStr, sync::Arc};

pub struct PageIter(Range<usize>);
impl PageIter {
	pub fn with_range(range: Range<usize>) -> Self {
		Self(range)
	}

	pub async fn new(page_idx: usize, provider: Arc<WebpageProvider>) -> anyhow::Result<Self> {
		let max_page_count = {
			let url = reqwest::Url::parse(Self::base_url())?;
			let response = provider.fetch(url)?.await?;
			let body = response.text().await?;
			Self::find_max_page_count(&body)?
		};
		Ok(Self(page_idx..max_page_count))
	}

	fn base_url() -> &'static str {
		"https://www.dndbeyond.com/monsters"
	}

	fn next_url(&self) -> String {
		format!("{}?page={}&sort=cr", Self::base_url(), self.0.start + 1)
	}

	fn find_max_page_count(html: &str) -> anyhow::Result<usize> {
		static PAGINATION_SELECTOR: [&'static str; 10] = [
			"body",
			"#site",
			"#site-main",
			".container",
			"#content",
			".primary-content",
			".listing-container",
			".listing-footer",
			".b-pagination",
			"ul.b-pagination-list",
		];
		let html = scraper::Html::parse_document(&html);

		let selector = scraper::Selector::parse(&PAGINATION_SELECTOR.join(" > ")).unwrap();
		let pagination_list = html.select(&selector).next().unwrap();

		let selector_item = scraper::Selector::parse(r#"li.b-pagination-item"#).unwrap();
		let paginator_item_count = pagination_list.select(&selector_item).count();
		// we need the last true page item, aka the second to last pagination item (last is the `Next` button).
		// Second to last is `length - 2`, because last is `length - 1`.
		let last_page_item = pagination_list
			.select(&selector_item)
			.nth(paginator_item_count - 2)
			.unwrap();

		let selector_label = scraper::Selector::parse(r#"a.b-pagination-item"#).unwrap();
		let last_page_label = last_page_item.select(&selector_label).next().unwrap();
		let last_page = last_page_label.inner_html().parse::<usize>()?;

		Ok(last_page)
	}

	pub fn next(&mut self) -> Option<String> {
		if self.0.start < self.0.end {
			let url = self.next_url();
			self.0.start += 1;
			Some(url)
		} else {
			None
		}
	}
}

pub struct CreatureListingPage(scraper::Html);
impl From<&String> for CreatureListingPage {
	fn from(body: &String) -> Self {
		Self(scraper::Html::parse_document(body))
	}
}
impl CreatureListingPage {
	pub fn list<'doc>(&'doc self) -> CreatureList<'doc> {
		let s_primary_content = scraper::Selector::parse(
			r#"body > #site > #site-main > .container > #content > .primary-content"#,
		)
		.unwrap();
		let primary_content = self.0.select(&s_primary_content).next().unwrap();

		let s_listings =
			scraper::Selector::parse(r#".listing-container > .listing-body > ul.listing"#).unwrap();
		let listings = primary_content.select(&s_listings).next().unwrap();
		CreatureList(listings)
	}
}

pub struct CreatureList<'doc>(scraper::ElementRef<'doc>);
impl<'doc> CreatureList<'doc> {
	pub fn children(&self) -> Vec<CreatureRowHtml<'doc>> {
		let s_info = scraper::Selector::parse(r#".info"#).unwrap();
		self.0.select(&s_info).map(CreatureRowHtml::from).collect()
	}
}

pub struct CreatureRowHtml<'doc>(scraper::ElementRef<'doc>);
impl<'doc> From<scraper::ElementRef<'doc>> for CreatureRowHtml<'doc> {
	fn from(html: scraper::ElementRef<'doc>) -> Self {
		Self(html)
	}
}
impl<'doc> CreatureRowHtml<'doc> {
	pub fn title_block(&self) -> TitleBlock<'doc> {
		let s_name_src = scraper::Selector::parse(r#".monster-name"#).unwrap();
		let name_and_src = self.0.select(&s_name_src).next().unwrap();
		TitleBlock(name_and_src)
	}

	pub fn challenge_rating(&self) -> Option<usize> {
		let s_cr = scraper::Selector::parse(r#".monster-challenge > span"#).unwrap();
		let cr = self.0.select(&s_cr).next();
		cr.map(|cr| cr.inner_html().parse::<usize>().ok()).flatten()
	}

	pub fn kind(&self) -> String {
		let s_type = scraper::Selector::parse(r#".monster-type > span.type"#).unwrap();
		let kind_html = self.0.select(&s_type).next().unwrap();
		kind_html.inner_html()
	}

	pub fn size(&self) -> String {
		let s_size = scraper::Selector::parse(r#".monster-size > span"#).unwrap();
		let size = self.0.select(&s_size).next().unwrap();
		size.inner_html()
	}
}

pub struct TitleBlock<'doc>(scraper::ElementRef<'doc>);
impl<'doc> TitleBlock<'doc> {
	pub fn expand(self) -> (String, String, PathBuf) {
		let source_book = self.source_book();
		let name_link = self.name_link();
		(name_link.name(), source_book, name_link.url())
	}

	pub fn name_link(&self) -> TitleBlockNameLink {
		let s_name = scraper::Selector::parse(r#"span.name > a.link"#).unwrap();
		TitleBlockNameLink(self.0.select(&s_name).next().unwrap())
	}

	pub fn source_book(&self) -> String {
		let s_source = scraper::Selector::parse(r#"span.source"#).unwrap();
		let source = self.0.select(&s_source).next().unwrap();
		source.inner_html()
	}
}

pub struct TitleBlockNameLink<'doc>(scraper::ElementRef<'doc>);
impl<'doc> TitleBlockNameLink<'doc> {
	pub fn name(&self) -> String {
		self.0.inner_html()
	}

	pub fn url(&self) -> PathBuf {
		PathBuf::from(self.0.value().attr("href").unwrap())
	}
}

#[derive(Debug, Clone)]
pub struct CreatureListing {
	pub(crate) name: String,
	pub(crate) source_book: String,
	pub(crate) url: PathBuf,
	pub(crate) challenge_rating: Option<usize>,
	pub(crate) kind: String,
	pub(crate) size: String,
}
impl<'doc> From<CreatureRowHtml<'doc>> for CreatureListing {
	fn from(row: CreatureRowHtml<'doc>) -> Self {
		let (name, source_book, url) = row.title_block().expand();
		Self {
			name,
			url,
			source_book,
			challenge_rating: row.challenge_rating(),
			kind: row.kind(),
			size: row.size(),
		}
	}
}
impl CreatureListing {
	pub fn name(&self) -> &String {
		&self.name
	}

	pub async fn fetch_full(self, provider: &Arc<WebpageProvider>) -> anyhow::Result<Creature> {
		let full_url = format!("https://www.dndbeyond.com{}", self.url.to_str().unwrap());
		let response = provider
			.fetch(full_url)?
			.await
			.context(format!("fetching creature {:?}", self.name))?;
		let body = response.text().await?;

		let tmp_output_path = PathBuf::from_str(&format!(
			"target/monsters/{}.html",
			self.url.file_name().unwrap().to_str().unwrap()
		))?;
		if let Some(parent) = tmp_output_path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}
		tokio::fs::write(tmp_output_path, &body).await?;

		Creature::parse(self, body)
	}

	/// Queries all of the pages in the monster catalogue,
	/// sending the public metadata (each row in the listings of each page) to the channel.
	pub async fn fetch_all(
		provider: Arc<WebpageProvider>,
		send_creature: async_channel::Sender<Self>,
		page_range: Option<Range<usize>>,
	) -> anyhow::Result<()> {
		let mut parsing_tasks = Vec::new();

		let mut page_iter = match page_range {
			Some(range) => PageIter::with_range(range),
			None => PageIter::new(0, provider.clone()).await?,
		};

		// Iterate over all of the pages that exist
		while let Some(url_string) = page_iter.next() {
			let async_provider = provider.clone();
			let send_channel = send_creature.clone();
			parsing_tasks.push(tokio::task::spawn(async move {
				let url = reqwest::Url::parse(&url_string)?;
				let response = async_provider.fetch(url)?.await?;
				let body = response.text().await?;

				let page = CreatureListingPage(scraper::Html::parse_document(&body));
				let list_elements = page.list().children();
				for element in list_elements.into_iter() {
					let _ = send_channel.try_send(CreatureListing::from(element));
				}

				Ok(()) as anyhow::Result<()>
			}));
		}
		futures::future::join_all(parsing_tasks).await;
		Ok(())
	}

	pub async fn fetch_pages(
		provider: Arc<WebpageProvider>,
		page_range: Option<Range<usize>>,
	) -> anyhow::Result<(Vec<(reqwest::Url, String)>, Vec<anyhow::Error>)> {
		let mut parsing_tasks = Vec::new();

		let mut page_iter = match page_range {
			Some(range) => PageIter::with_range(range),
			None => PageIter::new(0, provider.clone()).await?,
		};

		// Iterate over all of the pages that exist
		while let Some(url_string) = page_iter.next() {
			let async_provider = provider.clone();
			parsing_tasks.push(tokio::task::spawn(async move {
				let url = reqwest::Url::parse(&url_string)?;
				let response = async_provider.fetch(url.clone())?.await?;
				let body = response.text().await?;
				Ok((url, body)) as anyhow::Result<(reqwest::Url, String)>
			}));
		}

		Ok(async_runtime::join_all(parsing_tasks).await?)
	}
}
