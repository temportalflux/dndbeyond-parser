pub struct Node(scraper::Selector);
impl Node {
	pub fn new(elements: &[&'static str]) -> Self {
		Self(scraper::Selector::parse(&elements.join(" > ")).unwrap())
	}

	pub fn apply_to<'a>(
		&self,
		element: &scraper::ElementRef<'a>,
	) -> Result<scraper::ElementRef<'a>, NoSuchElement> {
		element.select(&self.0).next().ok_or(NoSuchElement)
	}

	pub fn apply_doc<'a>(
		&self,
		element: &'a scraper::Html,
	) -> Result<scraper::ElementRef<'a>, NoSuchElement> {
		element.select(&self.0).next().ok_or(NoSuchElement)
	}

	pub fn get_iter<'node, 'html>(
		&'node self,
		element: &scraper::ElementRef<'html>,
	) -> scraper::element_ref::Select<'html, 'node> {
		element.select(&self.0)
	}
}

#[derive(thiserror::Error, Debug)]
pub struct NoSuchElement;
impl std::fmt::Display for NoSuchElement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "No such html element")
	}
}
