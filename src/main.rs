use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

type BoxError = Box<dyn std::error::Error>;

fn main() -> Result<(), BoxError> {
	let path = match std::env::args().collect::<Vec<_>>().pop() {
		Some(p) => p,
		None => {
			println!("Missing path argument.");
			return Ok(());
		}
	};
	let html_paths = gather_paths(&std::path::PathBuf::from(path))?;

	for html_path in html_paths.into_iter() {
		println!("Parsing {:?}", html_path);
		if let Ok(text) = std::fs::read_to_string(&html_path) {
			if let Ok(monster) = Monster::new().parse_html(html_parser::Dom::parse(&text)?) {
				println!("  Found monster");
			} else {
				println!("  Not a monster");
			}
		} else {
			println!("  Failed to parse text.");
		}
	}

	Ok(())
}

fn gather_paths(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, BoxError> {
	let mut htmls = vec![];
	if root.is_dir() {
		for entry in root.read_dir()? {
			let file_path = entry?.path();
			if let Some(ext) = file_path.extension() {
				if ext == "html" {
					htmls.push(file_path);
				}
			}
		}
	} else if let Some(ext) = root.extension() {
		if ext == "html" {
			htmls.push(root.to_path_buf());
		} else {
			println!("Path argument must be a directory or .html file.");
			return Ok(vec![]);
		}
	}
	Ok(htmls)
}

#[derive(Debug, Clone)]
enum FieldId {
	Kind(String),
	Id(String),
	Class(String),
	Index(usize),
}

enum FieldIdRef<Tstr: Into<String>> {
	Kind(Tstr),
	Id(Tstr),
	Class(Tstr),
}

impl<T: Into<String>> FieldIdRef<T> {
	fn into_owned(items: Vec<Self>) -> Vec<FieldId> {
		items.into_iter().map(|r| r.into()).collect()
	}
}

impl<T: Into<String>> Into<FieldId> for FieldIdRef<T> {
	fn into(self) -> FieldId {
		match self {
			Self::Kind(e) => FieldId::Kind(e.into()),
			Self::Id(e) => FieldId::Id(e.into()),
			Self::Class(e) => FieldId::Class(e.into()),
		}
	}
}

impl<T: Into<String>> FieldIdRef<T> {
	fn as_node(self) -> Node {
		Node::new(self)
	}
}

impl FieldId {
	fn as_node(self) -> Node {
		Node::new(self)
	}

	fn matches(&self, index: usize, element: &html_parser::Node) -> bool {
		use html_parser::Node::*;
		match (self, element) {
			(Self::Index(idx), _) => *idx == index,
			(Self::Kind(name_str), Element(element)) => element.name == *name_str,
			(Self::Id(id_str), Element(element)) => {
				element.id.is_some() && *element.id.as_ref().unwrap() == *id_str
			}
			(Self::Class(name), Element(element)) => element.classes.contains(name),
			(_, _) => false,
		}
	}

	fn find_node_in(&self, nodes: &Vec<html_parser::Node>) -> Option<html_parser::Node> {
		nodes
			.iter()
			.enumerate()
			.find(|(i, dom_node)| self.matches(*i, dom_node))
			.map(|(_, node)| node.clone())
	}
}

struct Node {
	path: Vec<FieldId>,
	kind: Option<NodeKind>,
}

type NodeFieldParser = dyn Fn(&Vec<html_parser::Node>) -> String;

enum NodeKind {
	List(Vec<Node>),
	Field(String),
	FieldParser(String, Box<NodeFieldParser>),
}

impl Node {
	fn new<T: Into<FieldId>>(id: T) -> Self {
		Self {
			path: vec![id.into()],
			kind: None,
		}
	}

	fn next<T: Into<FieldId>>(mut self, id: T) -> Self {
		self.path.push(id.into());
		self
	}

	fn fork(mut self, nodes: Vec<Node>) -> Self {
		self.kind = Some(NodeKind::List(nodes));
		self
	}

	fn as_text(self) -> Self {
		self.next(FieldId::Index(0))
	}

	fn mark_field<T: Into<String>>(mut self, name: T) -> Self {
		self.kind = Some(NodeKind::Field(name.into()));
		self
	}

	fn parse<T: Into<String>>(mut self, name: T, parser: Box<NodeFieldParser>) -> Self {
		self.kind = Some(NodeKind::FieldParser(name.into(), parser));
		self
	}
}

impl Node {
	fn find_node_in(&self, dom_nodes: &Vec<html_parser::Node>) -> Option<html_parser::Node> {
		use html_parser::Node::*;
		let mut id_iter = self.path.iter();
		let mut dom_node = match id_iter.next() {
			Some(id) => match id.find_node_in(&dom_nodes) {
				Some(node) => node,
				None => {
					println!(
						"Node id {:?} does not exist in {:?}",
						id,
						dom_nodes
							.iter()
							.map(|node| {
								match node {
									Text(txt) => txt.clone(),
									Comment(_) => "comment".to_owned(),
									Element(element) => {
										format!("{:?}({:?})", element.name, element.id)
									}
								}
							})
							.collect::<Vec<_>>()
					);
					return None;
				}
			},
			None => return None,
		};

		for id in id_iter {
			if let Element(element) = dom_node {
				dom_node = match id.find_node_in(&element.children) {
					Some(node) => node,
					None => {
						println!(
							"Node id {:?} does not exist in {:?}",
							id,
							dom_nodes
								.iter()
								.map(|node| {
									match node {
										Text(txt) => txt.clone(),
										Comment(_) => "comment".to_owned(),
										Element(element) => {
											format!("{:?}({:?})", element.name, element.id)
										}
									}
								})
								.collect::<Vec<_>>()
						);
						return None;
					}
				};
			} else {
				println!("Node id {:?} has exhausted all children in the dom", id);
				return None;
			}
		}
		Some(dom_node)
	}

	fn parse_values(&self, dom_nodes: &Vec<html_parser::Node>) -> HashMap<String, String> {
		use html_parser::Node::*;
		let node = self.find_node_in(dom_nodes);
		if node.is_none() {
			println!("Failed to find node for path: {:?}", self.path);
		}
		let mut named_values: HashMap<String, String> = HashMap::new();
		match (&self.kind, &node) {
			(Some(NodeKind::List(children)), Some(Element(element))) => {
				for child in children.iter() {
					for (name, value) in child.parse_values(&element.children).into_iter() {
						named_values.insert(name, value);
					}
				}
			}
			(Some(NodeKind::Field(name)), Some(Text(txt))) => {
				named_values.insert(name.clone(), txt.clone());
			}
			(Some(NodeKind::FieldParser(name, parser)), Some(Element(element))) => {
				named_values.insert(name.clone(), parser(&element.children));
			}
			(_, _) => {}
		};
		named_values
	}
}

struct Monster {
	root: Node,
	named_values: HashMap<String, String>,
	size_type_alignment_regex: Regex,
}

fn extract_text(node: &html_parser::Node) -> String {
	use html_parser::Node::*;
	let mut str_builder = String::new();
	match node {
		Text(txt) => {
			str_builder.push_str(&txt);
		}
		Element(element) => {
			for node in element.children.iter() {
				str_builder.push_str(&extract_text(&node));
			}
		}
		Comment(_) => {}
	}
	str_builder
}

fn parse_description_block(element: &html_parser::Element, out: &mut String) {
	use html_parser::Node::*;
	if element.children.len() == 1 {
		if let Text(txt) = &element.children[0] {
			out.push_str(&txt);
			return;
		}
	}
	for (i, child) in element.children.iter().enumerate() {
		if i == 0 {
			out.push_str("<strong>");
			out.push_str(&extract_text(&child));
			out.pop();
			out.push_str("</strong>");
			out.push('\n');
		}
		else {
			out.push_str(&extract_text(&child));
		}
	}
}

fn parse_descriptions(nodes: &Vec<html_parser::Node>) -> String {
	use html_parser::Node::*;
	let mut str_builder = String::new();
	for node in nodes.iter() {
		match node {
			Text(txt) => {
				str_builder.push_str(&txt);
				str_builder.push('\n');
			}
			Element(element) => {
				parse_description_block(&element, &mut str_builder);
				str_builder.push('\n');
			}
			Comment(_) => {}
		}
	}
	str_builder.pop();
	str_builder
}

fn parse_actions(nodes: &Vec<html_parser::Node>) -> String {
	let mut str_builder = String::new();
	for node in nodes.iter() {
		str_builder.push_str(&extract_text(&node));
		str_builder.push('\n');
	}
	str_builder.pop();
	str_builder
}

impl Monster {
	fn new() -> Self {
		use FieldIdRef::*;
		Self {
			root: Kind("html")
				.as_node()
				.next(Kind("body"))
				.next(Id("site"))
				.next(Id("site-main"))
				.next(Class("container"))
				.next(Id("content"))
				.next(Class("primary-content"))
				.next(Class("monster-details"))
				.next(Class("more-info"))
				.next(Class("detail-content"))
				.fork(vec![
					Class("mon-stat-block").as_node().fork(vec![
						Class("mon-stat-block__header").as_node().fork(vec![
							Class("mon-stat-block__name")
								.as_node()
								.next(Class("mon-stat-block__name-link"))
								.as_text()
								.mark_field("name"),
							Class("mon-stat-block__meta")
								.as_node()
								.as_text()
								.mark_field("size-type-alignment"),
						]),
						Class("mon-stat-block__attributes").as_node().fork(vec![
							FieldId::Index(0)
								.as_node()
								.next(Class("mon-stat-block__attribute-value"))
								.fork(vec![
									Class("mon-stat-block__attribute-data-value")
										.as_node()
										.as_text()
										.mark_field("armor-class"),
									Class("mon-stat-block__attribute-data-extra")
										.as_node()
										.as_text()
										.mark_field("armor-class-context"),
								]),
							FieldId::Index(1)
								.as_node()
								.next(Class("mon-stat-block__attribute-data"))
								.fork(vec![
									Class("mon-stat-block__attribute-data-value")
										.as_node()
										.as_text()
										.mark_field("hit-points"),
									Class("mon-stat-block__attribute-data-extra")
										.as_node()
										.as_text()
										.mark_field("hit-points-roll"),
								]),
							FieldId::Index(2)
								.as_node()
								.next(Class("mon-stat-block__attribute-data"))
								.next(Class("mon-stat-block__attribute-data-value"))
								.as_text()
								.mark_field("speed"),
						]),
						Class("mon-stat-block__stat-block")
							.as_node()
							.next(Class("ability-block"))
							.fork(
								vec!["str", "dex", "con", "int", "wis", "cha"]
									.into_iter()
									.enumerate()
									.map(|(i, name)| {
										FieldId::Index(i)
											.as_node()
											.next(Class("ability-block__data"))
											.next(Class("ability-block__score"))
											.as_text()
											.mark_field(name)
									})
									.collect(),
							),
						Class("mon-stat-block__description-blocks")
							.as_node()
							.fork(vec![
								FieldId::Index(0)
									.as_node()
									.next(Class("mon-stat-block__description-block-content"))
									.parse("properties", Box::new(parse_descriptions)),
								FieldId::Index(1)
									.as_node()
									.next(Class("mon-stat-block__description-block-content"))
									.parse("actions", Box::new(parse_actions)),
							]),
					]),
					Class("more-info-content")
						.as_node()
						.next(Class("mon-details__description-block"))
						.next(Class("mon-details__description-block-content"))
						.parse("lore", Box::new(parse_descriptions)),
				]),
			named_values: HashMap::new(),
			size_type_alignment_regex: Regex::new(
				r"(?P<size>[\S]*) (?P<type>[\S]*).*, (?P<alignment>.*)",
			)
			.unwrap(),
		}
	}

	fn parse_html(mut self, html: html_parser::Dom) -> Result<Self, BoxError> {
		self.named_values = self.root.parse_values(&html.children);
		println!("{:?}", self.named_values);
		println!("{}", self);
		Ok(self)
	}
}

impl Monster {
	fn value(&self, key: &str) -> Option<&String> {
		self.named_values.get(key)
	}

	fn parsed_value(&self, key: &str, regex: &Regex, group_name: &str) -> Option<String> {
		let value = match self.value(key) {
			Some(v) => v,
			None => return None,
		};
		let captures = match regex.captures(&value) {
			Some(v) => v,
			None => return None,
		};
		let named_value = match captures.name(group_name) {
			Some(v) => v,
			None => return None,
		};
		Some(named_value.as_str().to_owned())
	}

	fn name(&self) -> Option<&String> {
		self.value("name")
	}

	fn size(&self) -> Option<String> {
		self.parsed_value(
			"size-type-alignment",
			&self.size_type_alignment_regex,
			"size",
		)
	}

	fn kind(&self) -> Option<String> {
		self.parsed_value(
			"size-type-alignment",
			&self.size_type_alignment_regex,
			"type",
		)
	}

	fn alignment(&self) -> Option<String> {
		self.parsed_value(
			"size-type-alignment",
			&self.size_type_alignment_regex,
			"alignment",
		)
	}

	fn armor_class(&self) -> Option<&String> {
		self.value("armor-class")
	}

	fn armor_class_context(&self) -> Option<&String> {
		self.value("armor-class-context")
	}

	fn hit_points(&self) -> Option<&String> {
		self.value("hit-points")
	}

	fn hit_points_roll(&self) -> Option<&String> {
		self.value("hit-points-roll")
	}

	fn speed(&self) -> Option<&String> {
		self.value("speed")
	}

	fn str(&self) -> Option<&String> {
		self.value("str")
	}

	fn dex(&self) -> Option<&String> {
		self.value("dex")
	}

	fn con(&self) -> Option<&String> {
		self.value("con")
	}

	fn int(&self) -> Option<&String> {
		self.value("int")
	}

	fn wis(&self) -> Option<&String> {
		self.value("wis")
	}

	fn cha(&self) -> Option<&String> {
		self.value("cha")
	}

	fn properties(&self) -> Option<&String> {
		self.value("properties")
	}

	fn actions(&self) -> Option<&String> {
		self.value("actions")
	}

	fn lore(&self) -> Option<&String> {
		self.value("lore")
	}
}

impl std::fmt::Display for Monster {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let unwrap = |o: Option<String>| o.unwrap_or("--".to_owned());
		let unwrap_ref = |o: Option<&String>| o.map(|s| s.to_owned()).unwrap_or("--".to_owned());
		write!(f, "Name: {}\n", unwrap_ref(self.name()))?;
		write!(
			f,
			"Armor Class: {} {}\n",
			unwrap_ref(self.armor_class()),
			unwrap_ref(self.armor_class_context())
		)?;
		write!(
			f,
			"Hit Points: {} {}\n",
			unwrap_ref(self.hit_points()),
			unwrap_ref(self.hit_points_roll())
		)?;
		write!(f, "Speed: {}\n", unwrap_ref(self.speed()))?;
		write!(f, "Size: {}\n", unwrap(self.size()))?;
		write!(f, "Type: {}\n", unwrap(self.kind()))?;
		write!(f, "Alignment: {}\n", unwrap(self.alignment()))?;
		write!(f, "Strength: {}\n", unwrap_ref(self.str()));
		write!(f, "Dexterity: {}\n", unwrap_ref(self.dex()));
		write!(f, "Constitution: {}\n", unwrap_ref(self.con()));
		write!(f, "Intelligence: {}\n", unwrap_ref(self.int()));
		write!(f, "Wisdom: {}\n", unwrap_ref(self.wis()));
		write!(f, "Charisma: {}\n", unwrap_ref(self.cha()));
		write!(f, "Properties: {}\n", unwrap_ref(self.properties()));
		write!(f, "Actions: {}\n", unwrap_ref(self.actions()));
		write!(f, "Lore: {}\n", unwrap_ref(self.lore()));
		Ok(())
	}
}
