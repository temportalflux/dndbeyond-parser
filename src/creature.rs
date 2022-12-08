use anyhow::Context;

use crate::{dndbeyond::creature_list::CreatureListing, utility::NoSuchElement};
use std::{path::PathBuf, str::FromStr};

fn strip_whitespace(text: String) -> anyhow::Result<String> {
	let strip_whitespace_r = regex::Regex::new(r"^[ \n\t]+(.*?)[ \t\n]+$")?;
	if let Some(captures) = strip_whitespace_r.captures(&text) {
		if let Some(group) = captures.get(1) {
			return Ok(group.as_str().to_owned());
		}
	}
	Ok(text)
}

#[derive(Clone)]
pub struct DiceRoll(u32, u32, Option<i32>);
impl std::fmt::Debug for DiceRoll {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "DiceRoll({})", self)
	}
}
impl std::fmt::Display for DiceRoll {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.2.as_ref() {
			Some(bonus) => write!(f, "{}d{}{:+}", self.0, self.1, bonus),
			None => write!(f, "{}d{}", self.0, self.1),
		}
	}
}

#[derive(Debug)]
pub struct Creature {
	name: String,
	source_book: String,
	url: PathBuf,
	challenge_rating: Option<usize>,
	kind: String,
	size: String,
	alignment: String,
	armor_class: (u32, Option<String>),
	hit_points: (u32, Option<DiceRoll>),
	speeds: Vec<(u32, Option<String>, Option<String>)>,
}
impl Creature {
	pub fn parse(listing: CreatureListing, html: String) -> anyhow::Result<Self> {
		let CreatureListing {
			name: _,
			source_book,
			url: _,
			challenge_rating,
			kind: _,
			size: _,
		} = listing;
		let document = scraper::Html::parse_document(&html);

		let page = CreaturePage::from_document(&document);

		let stat_block = page.stat_block();
		let (name, url, size, kind, alignment) = {
			let header = stat_block.header();
			let (name, url) = {
				let name_link = header.name_link();
				let name = name_link.name()?;
				let url = name_link.url()?;
				(name, url)
			};
			let (size, kind, alignment) = header.meta().expand()?;
			(name, url, size, kind, alignment)
		};
		let (armor_class, hit_points, speeds) = {
			let attributes = stat_block.attributes()?;
			let armor_class = attributes.armor_class()?;
			let hit_points = attributes.hit_points()?;
			let speeds = attributes.speeds()?;
			(armor_class, hit_points, speeds)
		};

		let creature = Self {
			name,
			source_book,
			url,
			challenge_rating,
			kind,
			size,
			alignment,
			armor_class,
			hit_points,
			speeds,
		};
		log::debug!("{creature:?}");
		Ok(creature)
	}
}

struct CreaturePage<'doc>(scraper::ElementRef<'doc>);
impl<'doc> CreaturePage<'doc> {
	pub fn from_document(document: &'doc scraper::Html) -> CreaturePage<'doc> {
		let s_primary_content = scraper::Selector::parse(
			r#"body > #site > #site-main > .container > #content > .primary-content"#,
		)
		.unwrap();
		let s_content =
			scraper::Selector::parse(r#".monster-details > div > .detail-content"#).unwrap();
		let primary_content = document.select(&s_primary_content).next().unwrap();
		let content = primary_content.select(&s_content).next().unwrap();
		Self(content)
	}

	pub fn stat_block(&self) -> StatBlock<'doc> {
		let s_stat_block = scraper::Selector::parse(r#".mon-stat-block"#).unwrap();
		StatBlock(self.0.select(&s_stat_block).next().unwrap())
	}
}

struct StatBlock<'doc>(scraper::ElementRef<'doc>);
impl<'doc> StatBlock<'doc> {
	pub fn header(&self) -> StatBlockHeader<'doc> {
		let s_header = scraper::Selector::parse(r#".mon-stat-block__header"#).unwrap();
		StatBlockHeader(self.0.select(&s_header).next().unwrap())
	}

	pub fn attributes(&self) -> anyhow::Result<Attributes> {
		let s_attributes = scraper::Selector::parse(r#".mon-stat-block__attributes"#).unwrap();
		Attributes::from(self.0.select(&s_attributes).next().unwrap())
	}

	pub fn stats(&self) -> Stats<'doc> {
		let s_stats = scraper::Selector::parse(r#".mon-stat-block__stat-block"#).unwrap();
		Stats(self.0.select(&s_stats).next().unwrap())
	}

	pub fn tidbits(&self) -> Tidbits<'doc> {
		let s_tidbits = scraper::Selector::parse(r#".mon-stat-block__tidbits"#).unwrap();
		Tidbits(self.0.select(&s_tidbits).next().unwrap())
	}
}

struct StatBlockHeader<'doc>(scraper::ElementRef<'doc>);
impl<'doc> StatBlockHeader<'doc> {
	pub fn name_link(&self) -> StatBlockHeaderNameLink<'doc> {
		let selector =
			scraper::Selector::parse(r#".mon-stat-block__name > a.mon-stat-block__name-link"#)
				.unwrap();
		StatBlockHeaderNameLink(self.0.select(&selector).next().unwrap())
	}

	pub fn meta(&self) -> StatBlockHeaderMeta<'doc> {
		let selector = scraper::Selector::parse(r#".mon-stat-block__meta"#).unwrap();
		StatBlockHeaderMeta(self.0.select(&selector).next().unwrap())
	}
}

struct StatBlockHeaderNameLink<'doc>(scraper::ElementRef<'doc>);
impl<'doc> StatBlockHeaderNameLink<'doc> {
	pub fn name(&self) -> anyhow::Result<String> {
		Ok(strip_whitespace(self.0.inner_html())?)
	}

	pub fn url(&self) -> anyhow::Result<PathBuf> {
		Ok(PathBuf::from_str(
			self.0.value().attr("href").ok_or(NoSuchElement)?,
		)?)
	}
}

struct StatBlockHeaderMeta<'doc>(scraper::ElementRef<'doc>);
impl<'doc> StatBlockHeaderMeta<'doc> {
	pub fn expand(&self) -> anyhow::Result<(String, String, String)> {
		use verbal_expr::Expression::Verex as Group;
		let regex = verbal_expr::start_of_line()
			.capture_expr(Group(&verbal_expr::anything_but(" ")))
			.then(" ")
			.capture_expr(Group(&verbal_expr::anything()))
			.then(", ")
			.capture_expr(Group(&verbal_expr::anything()))
			.compile()?;
		let text = self.0.inner_html();
		let captures = regex
			.captures(&text)
			.ok_or(NoSuchElement)
			.context("parse stat-block meta text")?;
		let size = captures
			.get(1)
			.ok_or(NoSuchElement)
			.context("parse creature size")?
			.as_str()
			.to_owned();
		let kind = captures
			.get(2)
			.ok_or(NoSuchElement)
			.context("parse creature type")?
			.as_str()
			.to_owned();
		let alignment = captures
			.get(3)
			.ok_or(NoSuchElement)
			.context("parse alignment")?
			.as_str()
			.to_owned();
		Ok((size, kind, alignment))
	}

	pub fn size(&self) -> anyhow::Result<String> {
		Ok(self.expand()?.0)
	}

	pub fn kind(&self) -> anyhow::Result<String> {
		Ok(self.expand()?.1)
	}

	pub fn alignment(&self) -> anyhow::Result<String> {
		Ok(self.expand()?.2)
	}
}

struct Attributes {
	armor_class: (String, Option<String>),
	hit_points: (String, Option<String>),
	speed: String,
}
impl Attributes {
	fn from<'doc>(html: scraper::ElementRef<'doc>) -> anyhow::Result<Self> {
		let s_attr = scraper::Selector::parse(r#".mon-stat-block__attribute"#).unwrap();
		let s_label = scraper::Selector::parse(r#".mon-stat-block__attribute-label"#).unwrap();
		let s_value = scraper::Selector::parse(r#".mon-stat-block__attribute-value"#).unwrap();
		let s_data = scraper::Selector::parse(r#".mon-stat-block__attribute-data"#).unwrap();
		let s_data_value =
			scraper::Selector::parse(r#".mon-stat-block__attribute-data-value"#).unwrap();
		let s_data_extra =
			scraper::Selector::parse(r#".mon-stat-block__attribute-data-extra"#).unwrap();

		let mut armor_class = (String::default(), None);
		let mut hit_points = (String::default(), None);
		let mut speed = String::default();
		for element in html.select(&s_attr) {
			let label = element.select(&s_label).next().unwrap();
			match label.inner_html().as_str() {
				"Armor Class" => {
					let value = element.select(&s_value).next().unwrap();
					let datum = value.select(&s_data_value).next().unwrap();
					let ac = datum.inner_html();
					let source = match value.select(&s_data_extra).next() {
						Some(extra) => Some(extra.inner_html()),
						None => None,
					};
					armor_class = (ac, source);
				}
				"Hit Points" => {
					let data = element.select(&s_data).next().unwrap();
					let datum = data.select(&s_data_value).next().unwrap();
					let hp = datum.inner_html();
					let dice = match data.select(&s_data_extra).next() {
						Some(extra) => Some(extra.inner_html()),
						None => None,
					};
					hit_points = (hp, dice);
				}
				"Speed" => {
					let data = element.select(&s_data).next().unwrap();
					let values_html = data.select(&s_data_value).next().unwrap();
					speed = values_html.inner_html();
				}
				_ => {}
			}
		}
		Ok(Self {
			armor_class,
			hit_points,
			speed,
		})
	}

	pub fn armor_class(&self) -> anyhow::Result<(u32, Option<String>)> {
		Ok((
			strip_whitespace(self.armor_class.0.clone())?.parse::<u32>()?,
			match &self.armor_class.1 {
				Some(text) => Some(strip_whitespace(text.clone())?),
				None => None,
			},
		))
	}

	pub fn hit_points(&self) -> anyhow::Result<(u32, Option<DiceRoll>)> {
		use verbal_expr::Expression::Verex as Group;
		let dice_regex = verbal_expr::find("(")
			.capture_expr(Group(&verbal_expr::digit().repeat_once_or_more()))
			.then("d")
			.capture_expr(Group(&verbal_expr::digit().repeat_once_or_more()))
			.maybe_expr(Group(
				&verbal_expr::find(" ")
					.capture_expr(Group(&verbal_expr::find("+").or_find("-")))
					.then(" ")
					.capture_expr(Group(&verbal_expr::digit().repeat_once_or_more())),
			))
			.then(")")
			.compile()?;
		Ok((
			strip_whitespace(self.hit_points.0.clone())?.parse::<u32>()?,
			match &self.hit_points.1 {
				Some(text) => {
					let text = strip_whitespace(text.clone())?;
					match dice_regex.captures(&text) {
						Some(capture) => {
							let die_count = capture
								.get(1)
								.map(|item| item.as_str().parse::<u32>().ok())
								.flatten()
								.unwrap_or(0);
							let die_kind = capture
								.get(2)
								.map(|item| item.as_str().parse::<u32>().ok())
								.flatten()
								.unwrap_or(0);
							let bonus_sign = capture
								.get(3)
								.map(|item| match item.as_str() {
									"+" => Some(1),
									"-" => Some(-1),
									_ => None,
								})
								.flatten()
								.unwrap_or(0);
							let bonus = capture
								.get(4)
								.map(|item| item.as_str().parse::<i32>().ok())
								.flatten();
							Some(DiceRoll(die_count, die_kind, bonus.map(|v| v * bonus_sign)))
						}
						None => None,
					}
				}
				None => None,
			},
		))
	}

	pub fn speeds(&self) -> anyhow::Result<Vec<(u32, Option<String>, Option<String>)>> {
		use verbal_expr::Expression::Verex as Group;

		let speed_regex = verbal_expr::maybe_expr(Group(
			&verbal_expr::capture_expr(Group(&verbal_expr::anything_but(" "))).then(" "),
		))
		.capture_expr(Group(&verbal_expr::digit().repeat_once_or_more()))
		.then(" ft.")
		.maybe_expr(Group(
			&verbal_expr::find(" (")
				.capture_expr(Group(&verbal_expr::word()))
				.find(")"),
		))
		.compile()?;

		let all_speeds = strip_whitespace(self.speed.clone())?;
		Ok(all_speeds
			.split(",")
			.filter_map(|speed_str| speed_regex.captures(&speed_str))
			.map(|capture| {
				let speed_type = capture.get(1).map(|item| item.as_str().to_owned());
				let distance = capture
					.get(2)
					.map(|item| item.as_str().parse::<u32>().ok())
					.flatten()
					.unwrap_or(0);
				let subtype = capture.get(3).map(|item| item.as_str().to_owned());
				(distance, speed_type, subtype)
			})
			.collect::<Vec<_>>())
	}
}

struct Stats<'doc>(scraper::ElementRef<'doc>);

struct Tidbits<'doc>(scraper::ElementRef<'doc>);
